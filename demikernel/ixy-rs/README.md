Rust Bindings for DPDK
=======================

[![Join us on Slack!](https://img.shields.io/badge/chat-on%20Slack-e01563.svg)](https://join.slack.com/t/demikernel/shared_invite/zt-t25ffjf9-2k7Y_594T8xn1GBWVYlQ2g)

This crate provides Rust bindings for [DPDK](https://www.dpdk.org/). The
following devices are supported:

- [x] Mellanox ConnextX-3 Pro
- [x] Mellanox ConnectX-4
- [x] Mellanox ConnextX-5

Building and Running
---------------------

**1. Clone This Repository**
```
export WORKDIR=$HOME                                  # Change this to whatever you want.
cd $WORKDIR                                           # Switch to working directory.
git clone https://github.com/demikernel/dpdk-rs.git   # Clone.
```

**2. Setup Build Environment (Optional)**

>  Set this if DPDK is not installed system wide.

```
export PKG_CONFIG_PATH=/path/to/dpdk/pkgconfig
```

**3. Build Rust Bindings for DPDK**
```
cd $WORKDIR/dpdk-rs    # Switch to working directory.
cargo build            # Build Rust bindings for DPDK.
```


Code of Conduct
---------------

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/)
or contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

Usage Statement
--------------

This project is a prototype. As such, we provide no guarantees that it will
work and you are assuming any risks with using the code. We welcome comments
and feedback. Please send any questions or comments to one of the following
maintainers of the project:

- [Irene Zhang](https://github.com/iyzhang) - [irene.zhang@microsoft.com](mailto:irene.zhang@microsoft.com)
- [Pedro Henrique Penna](https://github.com/ppenna) - [ppenna@microsoft.com](mailto:ppenna@microsoft.com)

> By sending feedback, you are consenting that it may be used  in the further
> development of this project.
