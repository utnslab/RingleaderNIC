# Ringleader

The FPGA prototype is implemented in Verilog code. All Ringleader components source file, including the packet parser, on-chip request buffer, FEO scheduler and reduction tree, is under the folder 
```bash
./fpga/common/rtl/ringleader/
```


Other NIC components, including DMA Engine, Ethernet MAC, and physical layer (PHY), is developed from the ["Corundum NIC"](https://github.com/corundum/corundum/) 
