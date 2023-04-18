# Ringleader

## Introduction
Ringleader is a new NIC architecture that utilizes novel hardware offloads to perform centralized orchestration. In RingLeader, scheduling and load balancing are performed in tandem by an efficient and precise novel hardware offload on the NIC, and core allocation is performed by a host datapath OS with information from a new OS-NIC interface. 

Ringleader's repository consists of three important components:

1. The FPGA prototype is implemented in Verilog (./hardware).
2. The user-space NIC driver is implemented in C. It provides DPDK-like kernel-bypass access to the NIC (./driver).
3. We integrated our NIC driver with the Demikernel libOS using Rust (./demikernel)

For more details about Ringleader, please check our paper. 

* [Ringleader: Efficiently Offloading Intra-Server Orchestration to NICs](https://utns.cs.utexas.edu/papers/ringleader.pdf)

## How to Build
### Step 1: Build the FPGA prototype
**1. Prerequisites**

To build and run our FPGA prototype, please ensure that you have the following:

1. A 100G Alveo U280 Data Center Accelerator Card.
2. Vivado 2021.1 installed with an activated license.

To verify that Vivado is installed correctly,
```bash
$ vivado -mode tcl 
  # Enter the Vivado TCl Command Palette
  Vivado% version // you should see 2021.1
  Vivado% quit 
```

**2. Build the FPGA project**

```bash
# this build process could take several hours.It generates the hardware bitstream.

bash build_fpga.sh 
```
**3. Reprogram the FPGA**

After the build, you should see your FPGA bitstream generated inside this folder:
```bash
./hardware/fpga/mqnic/AU280/fpga_100g/fpga/
```
Reprogram your FPGA using that bitstream through Vivado.


### Step 2: Build the NIC driver
```bash
bash build_driver.sh 
```

### Step 3: Build Demikernel

**1. Dependencies**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh 
source "$HOME/.cargo/env"

sudo apt-get install libclang-dev
sudo apt install clang

# We need this environment variable that points to Ringleader driver directory to build Demikernel:
export RINGLEADER_DRIVER_DIR=<RINGLEADER_REPO_PATH>/driver
```

**2. Build Demikernel with Ringleader**
```bash
#Please ensure you already built the Ringleader C driver
cd ./demikernel             # Switch to demikernel's working directory.
make all                    # Build demikernel using ringleader driver.
```

## How to Run

**Run Ringleader without Demikernel**

```bash
cd ./driver
# setup hugepages
sudo ./setup-hugetlbfs.sh

# setup vfio
sudo modprobe vfio-pci

# switch to root
sudo -s
echo 1234 1001 > /sys/bus/pci/drivers/vfio-pci/new_id

# exit root
ctr+D

# run ringleader throughput test (receiver side), replace 0000:ca:00.0 with your FPGA's PCIe address
sudo ./driver/ixy-throughput 0000:ca:00.0 
```

**Run Ringleader with Demikernel**

```bash
cd ./driver
# setup hugepages
sudo ./setup-hugetlbfs.sh

# setup vfio
sudo modprobe vfio-pci

# switch to root
sudo -s
echo 1234 1001 > /sys/bus/pci/drivers/vfio-pci/new_id

# exit root
ctr+D

cd ../demikernel/demikernel

# config your local/remote ip and local/remote mac.
vi ./default.yaml

# replace membind and cpunodebind number with your nic local numa node
# run Ringleader with demikernel (receiver side)
sudo numactl --strict --membind=1 --cpunodebind=1 env LD_LIBRARY_PATH=$RINGLEADER_DRIVER_DIR MSS=9000 MTU=1500 NUM_ITERS=1000 BUFFERSIZE=64 DEBUG=no  ECHO_SERVER=yes src/target/release/examples/ixy ./default.yaml

```