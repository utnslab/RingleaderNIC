// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//! Defines functions to be called during the TCP connection termination process.

use super::super::state::{receiver::ReceiverState, sender::SenderState, ControlBlock};
use crate::{
    fail::Fail,
    runtime::{Runtime, RuntimeBuf},
};
use futures::FutureExt;
use std::{num::Wrapping, rc::Rc};

/// Await until our state changes to `ReceivedFin`. Then sends an ACK for the received FIN.
async fn sender_ack_fin<RT: Runtime>(cb: Rc<ControlBlock<RT>>) -> Result<!, Fail> {
    loop {
        // Wait until we receive a FIN.
        let (receiver_st, receiver_st_changed) = cb.receiver.state.watch();
        if receiver_st != ReceiverState::ReceivedFin {
            receiver_st_changed.await;
            continue;
        }

        // Wait for all data to be acknowledged.
        let (ack_seq, ack_seq_changed) = cb.receiver.ack_seq_no.watch();
        let recv_seq = cb.receiver.recv_seq_no.get();
        if ack_seq != recv_seq {
            ack_seq_changed.await;
            continue;
        }

        // Send ACK segment for FIN.
        cb.receiver.state.set(ReceiverState::AckdFin);
        let remote_link_addr = cb.arp.query(cb.remote.address()).await?;
        let mut header = cb.tcp_header();

        // ACK replies to FIN are special as their ack sequence number should be set to +1 the
        // received seq number even though there is no payload.
        header.ack = true;
        header.ack_num = recv_seq + Wrapping(1);
        cb.emit(header, RT::Buf::empty(), remote_link_addr);
    }
}

/// Spawns a future that awaits for sender status to change to Closed . Once status is Closed
/// sends FIN. Then goes back to a awaiting change until/if any further changes to our SenderState.
async fn sender_send_fin<RT: Runtime>(cb: Rc<ControlBlock<RT>>) -> Result<!, Fail> {
    loop {
        let (sender_st, sender_st_changed) = cb.sender.state.watch();
        match sender_st {
            SenderState::Open | SenderState::SentFin | SenderState::FinAckd => {
                sender_st_changed.await;
                continue;
            }
            SenderState::Closed => {
                // Wait for `sent_seq_no` to catch up to `unsent_seq_no` and
                // then send a FIN segment.
                let (sent_seq, sent_seq_changed) = cb.sender.sent_seq_no.watch();
                let unsent_seq = cb.sender.unsent_seq_no.get();

                if sent_seq != unsent_seq {
                    sent_seq_changed.await;
                    continue;
                }

                // TODO: When do we retransmit this?
                let remote_link_addr = cb.arp.query(cb.remote.address()).await?;
                let mut header = cb.tcp_header();
                header.seq_num = sent_seq;
                header.fin = true;
                cb.emit(header, RT::Buf::empty(), remote_link_addr);

                cb.sender.state.set(SenderState::SentFin);
            }
            SenderState::Reset => {
                let remote_link_addr = cb.arp.query(cb.remote.address()).await?;
                let mut header = cb.tcp_header();
                header.rst = true;
                cb.emit(header, RT::Buf::empty(), remote_link_addr);
                return Err(Fail::ConnectionAborted {});
            }
        }
    }
}

/// Awaits until connection terminates by our four-way handshake.
async fn close_wait<RT: Runtime>(cb: Rc<ControlBlock<RT>>) -> Result<!, Fail> {
    loop {
        // Wait until the FIN we sent has been ACKed.
        let (sender_st, sender_st_changed) = cb.sender.state.watch();
        if sender_st != SenderState::FinAckd {
            sender_st_changed.await;
            continue;
        }

        // Wait until we ACK the FIN that was sent to us.
        let (receiver_st, receiver_st_changed) = cb.receiver.state.watch();
        if receiver_st != ReceiverState::AckdFin {
            receiver_st_changed.await;
            continue;
        }

        // TODO: Wait for 2*MSL if active close.
        return Err(Fail::ConnectionAborted {});
    }
}

/// Launches various closures having to do with connection termination. Neither `sender_ack_fin`
/// nor `sender_send_fin` terminate so the only way to return is via `close_wait`.
pub async fn connection_terminated<RT: Runtime>(cb: Rc<ControlBlock<RT>>) -> Result<!, Fail> {
    futures::select_biased! {
        r = sender_ack_fin(cb.clone()).fuse() => r,
        r = sender_send_fin(cb.clone()).fuse() => r,
        r = close_wait(cb).fuse() => r,
    }
}
