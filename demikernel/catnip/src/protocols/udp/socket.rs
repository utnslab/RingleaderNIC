// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

use crate::protocols::ipv4;

//==============================================================================
// Constants & Structures
//==============================================================================

/// UDP Socket
#[derive(Debug)]
pub struct Socket {
    /// Local endpoint.
    local: Option<ipv4::Endpoint>,
    /// Remote endpoint.
    remote: Option<ipv4::Endpoint>,
}

//==============================================================================
// Associate Functions
//==============================================================================

// Associate functions for [Socket].
impl Socket {
    pub fn local(&self) -> Option<ipv4::Endpoint> {
        self.local
    }

    pub fn remote(&self) -> Option<ipv4::Endpoint> {
        self.remote
    }

    pub fn set_local(&mut self, local: Option<ipv4::Endpoint>) {
        self.local = local;
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Default trait implementation for [Socket].
impl Default for Socket {
    /// Creates a UDP socket with default values.
    fn default() -> Self {
        Self {
            local: None,
            remote: None,
        }
    }
}
