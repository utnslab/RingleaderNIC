`timescale 1ns / 1ps
`include "define.v"

module perf_monitor #
(
    // Width of AXI data bus in bits
    parameter APP_ELI_MASK_WIDTH = 2** `APP_ID_WIDTH
)
(
    input  wire                             clk,
    input  wire                             rst,

    /* output (scheduled) packet descriptor*/
    input wire [`APP_ID_WIDTH-1 : 0]        app_id,
    input wire                              enqueue_packet,
    input wire                              dequeue_packet,

    input wire                              reset_monitor,
    input wire                              config_monitor,
    input wire [4:0]                        config_scale_down_epoch_log,
    input wire [4:0]                        config_cong_dectect_epoch_log,
    input wire [3:0]                        config_scale_down_thresh,

    input wire                            arm_cong_monitor,
    input wire                            arm_scale_down_monitor,


    output reg                                 msg_en,
    output reg [`NIC_MSG_WIDTH - 1 : 0]        msg
);

reg [63:0] cycle_counter;

reg [31:0] absolute_queue_length;

reg [31:0] scale_down_acc_queue_length_counter;
reg [31:0] scale_down_epoch_counter;
reg [31:0] scale_down_epoch;
reg [3:0]  scale_down_trigger_reg;

reg [31:0] cong_dectect_epoch;
reg [31:0] cong_epoch_counter;

reg [31:0] queuing_epoch_0_pkc;
reg [31:0] queuing_epoch_1_pkc;
reg [31:0] dequeue_epoch_0_pkc;


reg msg_cong_en, msg_cong_inflight;
reg [`NIC_MSG_CONTENT_SIZE -1 : 0 ] measured_avg_queue_length;
reg msg_scale_down_en, msg_scale_down_inflight;


reg [4:0]                        config_scale_down_epoch_log_reg;
reg [4:0]                        config_cong_dectect_epoch_log_reg;

// use shenango policy, assert queueing delay if "packet enqueued in  epoch_0 is larger than packet that is dequeued in epoch_1"

always@(posedge clk) begin
    if(rst | reset_monitor) begin
        cycle_counter <= 0;
        scale_down_epoch_counter <= 0;
        measured_avg_queue_length <= 0;
        msg_scale_down_en <= 0;
        scale_down_epoch <= 32'hffffffff;
        cong_dectect_epoch <= 32'hffffffff;
        cong_epoch_counter <= 0;
        msg_cong_inflight <= 0;
        scale_down_trigger_reg <= 0;
        msg_scale_down_inflight <= 0;
        msg_cong_en <= 0;
        queuing_epoch_0_pkc <= 0;
        queuing_epoch_1_pkc <= 0;
        dequeue_epoch_0_pkc <= 0;
        absolute_queue_length <= 0;

        config_scale_down_epoch_log_reg <= 0;
        config_cong_dectect_epoch_log_reg <= 0;
        scale_down_acc_queue_length_counter <= 0;
    end
    else if(config_monitor) begin
        scale_down_epoch <= (32'h1 << config_scale_down_epoch_log);
        config_scale_down_epoch_log_reg <= config_scale_down_epoch_log;
        cong_dectect_epoch <= (32'h1 << config_cong_dectect_epoch_log);
        config_cong_dectect_epoch_log_reg <= config_cong_dectect_epoch_log;
        scale_down_trigger_reg <= config_scale_down_thresh;
    end
    else begin


        cycle_counter <= cycle_counter + 1;

        if(enqueue_packet && dequeue_packet) begin
            absolute_queue_length <= absolute_queue_length;
        end
        else if(enqueue_packet) begin
            absolute_queue_length <= absolute_queue_length + 1;
        end
        else if(dequeue_packet) begin
            absolute_queue_length <= absolute_queue_length - 1;
        end

        // scale down cal routine
        if(arm_scale_down_monitor) begin
            scale_down_epoch_counter <= 0;
            msg_scale_down_inflight <= 0;
            msg_scale_down_en <= 0;
            scale_down_acc_queue_length_counter <= 0;
        end
        else begin
            scale_down_epoch_counter <= scale_down_epoch_counter + 1;
            if(absolute_queue_length > scale_down_acc_queue_length_counter) begin
                scale_down_acc_queue_length_counter <= absolute_queue_length;
            end

            if(scale_down_epoch_counter >= scale_down_epoch) begin
                if(scale_down_acc_queue_length_counter  < scale_down_trigger_reg) begin
                    msg_scale_down_en <= 1;              
                end
                scale_down_acc_queue_length_counter <= 0;
                scale_down_epoch_counter <= 0;
            end

        end
        
        // measure congestion
        if(enqueue_packet && dequeue_packet) begin
            queuing_epoch_0_pkc <= queuing_epoch_0_pkc;
        end
        else if(enqueue_packet) begin
            queuing_epoch_0_pkc <= queuing_epoch_0_pkc + 1;
        end
        else if(dequeue_packet) begin
            queuing_epoch_0_pkc <= queuing_epoch_0_pkc - 1;
        end

        if(arm_cong_monitor) begin
            cong_epoch_counter <= 0;
            msg_cong_inflight <= 0;
            msg_cong_en <= 0;
            // queuing_epoch_0_pkc <= 0;
            queuing_epoch_1_pkc <= queuing_epoch_0_pkc;
            dequeue_epoch_0_pkc <= dequeue_packet ? 1 : 0;
        end
        else begin
             cong_epoch_counter <= cong_epoch_counter + 1;

            if(dequeue_packet) begin
                dequeue_epoch_0_pkc <= dequeue_epoch_0_pkc + 1;
            end

            if(cong_epoch_counter >= cong_dectect_epoch) begin

                if(dequeue_epoch_0_pkc < queuing_epoch_1_pkc && config_cong_dectect_epoch_log_reg != 0) begin
                    msg_cong_en <= 1;
                end

                queuing_epoch_1_pkc <= queuing_epoch_0_pkc;

                dequeue_epoch_0_pkc <= dequeue_packet ? 1 : 0;
                cong_epoch_counter <= 0;
            end

        end
       
        // generate NIC-CPU msg
        if(msg_scale_down_en && !msg_scale_down_inflight) begin
            msg_en <= 1;
            msg[`NIC_MSG_TYPE_OF   +: `NIC_MSG_TYPE_SIZE]  = `NIC_MSG_SCALE_DOWN_HINT;
            msg[`NIC_MSG_APP_ID_OF   +: `NIC_MSG_APP_ID_SIZE]  = app_id;
            msg[`NIC_MSG_CONTENT_OF   +: `NIC_MSG_CONTENT_SIZE] = 16'h2;
            msg_scale_down_en <= 0;
            msg_scale_down_inflight <= 1;
        end
        else if(msg_cong_en && !msg_cong_inflight) begin
            msg_en <= 1;
            msg[`NIC_MSG_TYPE_OF   +: `NIC_MSG_TYPE_SIZE]  = `NIC_MSG_CONG_HINT;
            msg[`NIC_MSG_APP_ID_OF   +: `NIC_MSG_APP_ID_SIZE]  = app_id;
            msg[`NIC_MSG_CONTENT_OF   +: `NIC_MSG_CONTENT_SIZE] = 16'h1;
            msg_cong_en <= 0;
            msg_cong_inflight <= 1;
        end
        else begin
            msg_en <= 0;
            msg <= 0;
        end


    end

end

wire if_trigger;
assign if_trigger = cong_epoch_counter >= cong_dectect_epoch;

// ila_0 monitor_perf_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(enqueue_packet), // input wire [0:0] probe0  
// 	.probe1({enqueue_packet, dequeue_packet, arm_cong_monitor, arm_scale_down_monitor, config_cong_dectect_epoch_log, msg_cong_en, msg_cong_inflight, msg_en, queuing_epoch_0_pkc[7:0], queuing_epoch_1_pkc[7:0], dequeue_epoch_0_pkc[11:0], if_trigger, cong_epoch_counter[11:0], cong_dectect_epoch[11:0], config_monitor, config_cong_dectect_epoch_log_reg})
// );


endmodule