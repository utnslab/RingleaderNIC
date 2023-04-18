// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

//==============================================================================
// Constants & Structures
//==============================================================================

/// Control Options for UDP
#[derive(Clone, Debug)]
pub struct UdpOptions {
    /// Enable checksum offload on receiver side?
    rx_checksum: bool,
    /// Enable checksum offload on sender side?
    tx_checksum: bool,
}

//==============================================================================
// Associate Functions
//==============================================================================

/// Associate functions for [UdpOptions].
impl UdpOptions {
    /// Creates custom options for UDP.
    pub fn new(rx_checksum: bool, tx_checksum: bool) -> Self {
        Self {
            rx_checksum,
            tx_checksum,
        }
    }

    /// Returns whether or not checksum offload on receiver side is enabled.
    pub fn rx_checksum(&self) -> bool {
        self.rx_checksum
    }

    /// Returns whether or not checksum offload on sender side is enabled.
    pub fn tx_checksum(&self) -> bool {
        self.tx_checksum
    }
}

//==============================================================================
// Trait Implementations
//==============================================================================

/// Implementation of [Default] trait for [UdpOptions].
impl Default for UdpOptions {
    /// Creates default options for UDP.
    fn default() -> Self {
        UdpOptions {
            rx_checksum: false,
            tx_checksum: false,
        }
    }
}

//==============================================================================
// Unit Tests
//==============================================================================

#[cfg(test)]
mod tests {
    use super::UdpOptions;

    /// Tests instantiations flavors for [UdpOptions].
    #[test]
    fn test_udp_options() {
        //Default options.
        let options_default = UdpOptions::default();
        assert!(!options_default.rx_checksum());
        assert!(!options_default.tx_checksum());

        // Custom options.
        let options_custom = UdpOptions::new(true, true);
        assert!(options_custom.rx_checksum());
        assert!(options_custom.tx_checksum());
    }
}
