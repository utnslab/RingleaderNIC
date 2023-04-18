create_ip -name ila -vendor xilinx.com -library ip -version 6.2 -module_name ila_0

set_property -dict [list CONFIG.C_PROBE1_WIDTH {64} CONFIG.C_NUM_OF_PROBES {2}] [get_ips ila_0]

