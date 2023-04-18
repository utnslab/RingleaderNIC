We extend Demikernel’s
catnip libOS to support Ringleader NIC. 

Modifications to Demikernel: 
--------


**1.Support for RingLeader’s user space driver:**

We extended the catnip libOS to add support for RingLeader’s user space driver. Which is implemented in the following files:

 ### Related Files
    ./demikernel/src/catnip-libos/src  : rust library for RingLeader
    ./ixy-rs                           : Rust Bindings for the C driver


**2. Other modifications to Demikernel:**  

We extended Demikernel’s coroutine scheduler to enforce prioritized scheduling between different services’ coroutines; we add multi-core support and implement a core allocator inside Demikernel.

 ### Source Files
    ./catnip/src/libos.rs  : Demikernel libos
