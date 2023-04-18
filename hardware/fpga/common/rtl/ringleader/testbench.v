`timescale 1ns / 1ps
`include "define.v"

module testbench;

reg  clk;
reg rst;
reg start;
always begin
    clk = ~clk; 
    #2;
end

localparam QUEUE_INDEX_WIDTH = 6;
localparam QUEUE_PTR_WIDTH = 8;

localparam AXIS_DATA_WIDTH = 512;
localparam AXIS_KEEP_WIDTH = AXIS_DATA_WIDTH / 8;
localparam REG_DATA_WIDTH = 32;
localparam REG_ADDR_WIDTH = 16;
localparam LEN_WIDTH = 16;

parameter RAM_ADDR_WIDTH = 20;

parameter RX_RAM_SIZE = 524288;


reg [QUEUE_INDEX_WIDTH-1:0]       s_axis_cpu_msg_queue;
reg                               s_axis_cpu_msg_valid;
reg [`CPU_MSG_WIDTH-1:0]          s_axis_cpu_msg;

reg [QUEUE_INDEX_WIDTH-1:0]       ini_axis_cpu_msg_queue;
reg                               ini_axis_cpu_msg_valid;
reg [`CPU_MSG_WIDTH-1:0]          ini_axis_cpu_msg;

reg [REG_ADDR_WIDTH-1:0]            ctrl_reg_wr_addr;
reg [REG_DATA_WIDTH-1:0]            ctrl_reg_wr_data;
reg                                 ctrl_reg_wr_en;

wire [1:0]                          ram_wr_cmd_valid ;  
reg  [1:0]                          ram_wr_cmd_valid_reg ;  

initial begin
    clk = 0;
    rst = 1;
    start = 0;
    
    #1000;
    rst = 0;

    #4;
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = 1;
    ctrl_reg_wr_addr = 16'h0094;
    #4;
    #4;
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = 2 * 4;
    ctrl_reg_wr_addr = 16'h0098;
    #4;
    #4;
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = 32'hc0a80165;
    ctrl_reg_wr_addr = 16'h0090;
    #4;
    #4;
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = 16'h0003;
    ctrl_reg_wr_addr = 16'h0088;
    #4;
    #4;
    // config application priority, port match action table
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = {16'h0012, 4'h1, 8'h1, 4'h0};
    ctrl_reg_wr_addr = 16'h0080;
    #4;
    #4;
    // config application priority, port match action table
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = {16'h0034, 4'h2, 8'h2, 4'h0};
    ctrl_reg_wr_addr = 16'h0080;
    #4;
    #4;
    // reset monitor 
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = {8'h5, 8'h10, 8'h1, 4'h2};
    ctrl_reg_wr_addr = 16'h0080;
    #4;
    #4;
    // config monitor 
    ctrl_reg_wr_en = 1;
    ctrl_reg_wr_data = {4'h2, 8'h5, 8'h5, 8'h1, 4'h1};
    ctrl_reg_wr_addr = 16'h0080;
    #4;
    ctrl_reg_wr_en = 0;

    // config app1's queue
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 0;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h1, 8'h1, 4'h3};
        // ini_axis_cpu_msg = 19  + (32'h1<<12);
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 1;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h1, 8'h1, 4'h3};
        // ini_axis_cpu_msg = 19  + (32'h1<<12);
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 2;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h1, 8'h1, 4'h3};
        // ini_axis_cpu_msg = 19  + (32'h1<<12);
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 3;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h1, 8'h1, 4'h3};
        // ini_axis_cpu_msg = 19  + (32'h1<<12);

     // config app0's queue
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 0;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h0, 8'h2, 4'h3};
        // ini_axis_cpu_msg = 35;
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 1;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h0, 8'h2, 4'h3};
        // ini_axis_cpu_msg = 35;
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 2;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h0, 8'h2, 4'h3};
        // ini_axis_cpu_msg = 35;
    #4;
    ini_axis_cpu_msg_valid = 1;
    ini_axis_cpu_msg_queue = 3;
    ini_axis_cpu_msg = 17;
        #4;
        ini_axis_cpu_msg = {4'h1, 4'h4, 4'h0, 8'h2, 4'h3};
        // ini_axis_cpu_msg = 35;




    
    #4;
    ini_axis_cpu_msg_valid = 0;
    ini_axis_cpu_msg = 0;
    ini_axis_cpu_msg_queue = 0;



    #600;
    start = 1;
end



reg  [AXIS_DATA_WIDTH-1:0]    rx_axis_tdata;
reg  [AXIS_KEEP_WIDTH-1:0]    rx_axis_tkeep;
reg                           rx_axis_tvalid;
wire                          rx_axis_tready;
reg                           rx_axis_tlast;
reg                           rx_axis_tuser;

  wire                              free_mem_ready;
wire                                free_mem_req;
wire [LEN_WIDTH - 1 : 0]            free_mem_size;
wire [RAM_ADDR_WIDTH-1 : 0]                       free_mem_addr;


wire [QUEUE_INDEX_WIDTH-1:0]       s_axis_update_queue;
wire                               s_axis_update_valid;
wire                               s_axis_update_ready;
wire [31:0]                        s_axis_hash;
wire [QUEUE_PTR_WIDTH-1:0]         s_axis_update_length;
wire [`APP_ID_WIDTH-1:0]           s_axis_update_app_id;
wire [RAM_ADDR_WIDTH -1 : 0]       s_axis_update_mem_addr;
wire [LEN_WIDTH - 1 : 0]           s_axis_update_mem_size;

reg cpu_msg_ready;
wire ready_interval = (cycle_counter % 2 == 0);

ringleader #
(
    /* MEMORY PARAMETER */
    // Width of AXI memory data bus in bits, normal is 512
    .AXI_DATA_WIDTH(AXIS_DATA_WIDTH),
    // Width of panic memory address bus in bits
    .AXI_ADDR_WIDTH(16),

    /*AXIS INTERFACE PARAMETER*/
    // Width of AXI stream interfaces in bits, normal is 512
    .AXIS_DATA_WIDTH(AXIS_DATA_WIDTH),
    .AXIS_KEEP_WIDTH(AXIS_KEEP_WIDTH),
    .AXIS_LAST_ENABLE(1),
    .AXIS_ID_ENABLE(0),
    .REG_DATA_WIDTH(REG_DATA_WIDTH),
    .REG_ADDR_WIDTH(REG_ADDR_WIDTH),
    .LEN_WIDTH(LEN_WIDTH),
    .SEG_COUNT(2),
    .SEG_DATA_WIDTH(512),
    .SEG_ADDR_WIDTH(13),
    .SEG_BE_WIDTH(64),
    .RAM_ADDR_WIDTH(RAM_ADDR_WIDTH),
    .DMA_CLIENT_LEN_WIDTH(16),
    .DMA_CLIENT_TAG_WIDTH(5),
    .RX_RAM_SIZE(RX_RAM_SIZE),
    .QUEUE_INDEX_WIDTH(QUEUE_INDEX_WIDTH),
    .QUEUE_PTR_WIDTH(QUEUE_PTR_WIDTH),
    .RX_HASH_ENABLE(1),
    .RX_CHECKSUM_ENABLE(1)

)
ringleader_inst
(
    .clk(clk),
    .rst(rst),

    /*
        * Control register interface
        */
    .ctrl_reg_wr_addr(ctrl_reg_wr_addr),
    .ctrl_reg_wr_data(ctrl_reg_wr_data),
    .ctrl_reg_wr_en(ctrl_reg_wr_en),
    .ctrl_reg_wr_ack(),
    .ctrl_reg_rd_addr(0),
    .ctrl_reg_rd_en(0),
    .ctrl_reg_rd_data(),
    .ctrl_reg_rd_ack(),

    /*
        * Receive data input
        */
    .s_rx_axis_tdata(rx_axis_tdata),
    .s_rx_axis_tkeep(rx_axis_tkeep),
    .s_rx_axis_tvalid(rx_axis_tvalid),
    .s_rx_axis_tready(rx_axis_tready),
    .s_rx_axis_tlast(rx_axis_tlast),
    .s_rx_axis_tuser(rx_axis_tuser),

    .m_axis_rx_req_queue(s_axis_update_queue),
    .m_axis_rx_req_app_id(s_axis_update_app_id),
    .m_axis_rx_req_tag(),
    .m_axis_rx_req_valid(s_axis_update_valid),
    .m_axis_rx_req_ram_addr(s_axis_update_mem_addr),
    .m_axis_rx_req_len(s_axis_update_mem_size),
    .m_axis_rx_csum(),
    .m_axis_rx_hash(s_axis_hash),
    .m_axis_rx_req_ready(s_axis_update_ready && ready_interval),

    .ram_wr_cmd_be(),
    .ram_wr_cmd_addr(),
    .ram_wr_cmd_data(),
    .ram_wr_cmd_valid(ram_wr_cmd_valid),
    .ram_wr_cmd_ready(2'b11),
    .ram_wr_done(ram_wr_cmd_valid_reg),

    // .s_axis_update_queue(fifo_update_queue),
    // .s_axis_update_valid(fifo_update_valid),
    // .s_axis_update_length(1),
    // .s_axis_update_app_id(fifo_update_app_id),

    .s_axis_cpu_msg_queue(s_axis_cpu_msg_queue),
    .s_axis_cpu_msg_valid(s_axis_cpu_msg_valid),
    .s_axis_cpu_msg(s_axis_cpu_msg),


    .free_mem_ready(free_mem_ready),
    .free_mem_req(fifo_update_valid && free_mem_ready && cpu_msg_ready),
    .free_mem_size(free_mem_size),
    .free_mem_addr(free_mem_addr)

);

wire [QUEUE_INDEX_WIDTH-1:0]       fifo_update_queue;
wire                               fifo_update_valid;
wire [`APP_ID_WIDTH-1:0]           fifo_update_app_id;
wire [19 : 0]                      fifo_update_mem_addr;
wire [LEN_WIDTH - 1 : 0]           fifo_update_mem_size;

always @(*) begin
    cpu_msg_ready =1;
    if(ini_axis_cpu_msg_valid) begin
        s_axis_cpu_msg = ini_axis_cpu_msg;
        s_axis_cpu_msg_valid = ini_axis_cpu_msg_valid;
        s_axis_cpu_msg_queue = ini_axis_cpu_msg_queue;
    end
    else if(s_axis_hash  != 0 &&  s_axis_update_valid && ready_interval && s_axis_update_ready) begin
        cpu_msg_ready =0;
        if(s_axis_hash[`NIC_MSG_TYPE_OF   +: `NIC_MSG_TYPE_SIZE] ==  `NIC_MSG_SCALE_DOWN_HINT)
            s_axis_cpu_msg = (32'b1<<16) | ((32'b0 + s_axis_update_app_id)<<4)|  `CPU_MSG_ARM_SCALE_DOWN_MONITOR;
        else if(s_axis_hash[`NIC_MSG_TYPE_OF   +: `NIC_MSG_TYPE_SIZE] ==  `NIC_MSG_CONG_HINT)
            s_axis_cpu_msg = (32'b1<<16) | ((32'b0 + s_axis_update_app_id)<<4)|  `CPU_MSG_ARM_CONG_MONITOR;
        s_axis_cpu_msg_valid = 1;
        s_axis_cpu_msg_queue = s_axis_update_queue;
    end
    else begin
        s_axis_cpu_msg = (32'b1<<16) | ((32'b0 + fifo_update_app_id)<<4)|  5;
        s_axis_cpu_msg_valid = fifo_update_valid && free_mem_ready && cpu_msg_ready;
        s_axis_cpu_msg_queue = fifo_update_queue;
    end
end

axis_fifo #(
    .DEPTH(16),
    .DATA_WIDTH(128),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0),
    .PIPELINE_OUTPUT(512)
)
feedback_delay (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata({s_axis_update_app_id, s_axis_update_queue, s_axis_update_mem_addr, s_axis_update_mem_size}),
    .s_axis_tvalid(s_axis_update_valid && ready_interval),
    .s_axis_tready(s_axis_update_ready),

    // AXI output
    .m_axis_tdata({fifo_update_app_id, fifo_update_queue, free_mem_addr, free_mem_size}),
    .m_axis_tvalid(fifo_update_valid),
    .m_axis_tready(free_mem_ready && cpu_msg_ready)
);

reg [31:0] test_counter = 0;
reg if_send;
reg [63:0] counter;
reg [63:0] c_counter;
reg [63:0] cycle_counter;
reg [63:0] byte_counter;

wire [15:0] packet_len;
wire [15:0] header_length;
assign packet_len = 4;
assign header_length = (packet_len)*64 - 14;

reg [4:0] flow_id;
always@(*) begin

    rx_axis_tvalid = 0;
    rx_axis_tlast = 0;
    if(start && counter < 32) begin
        if(if_send) begin
            rx_axis_tvalid = 1;
            if(c_counter == 0) begin
                if(cycle_counter % 2 == 1) begin
                    rx_axis_tdata = 512'h1514131211100F0E0D0C0B0A0908070605040302010081B90801020001006501A8C06401A8C0B7B5114000400000F2050045000855545352515AD5D4D3D2D1DA; // udp header for app 1
                    rx_axis_tdata[37*8 +: 8] = 8'h34;
                    rx_axis_tdata[36*8 +: 8] = 0;
                end
                else begin
                    rx_axis_tdata = 512'h1514131211100F0E0D0C0B0A0908070605040302010081B90801020001006501A8C06401A8C1B7B5114000400000F2050045000855545352515AD5D4D3D2D1DA; // udp header for app 0
                    rx_axis_tdata[37*8 +: 8] = 8'h12;
                    rx_axis_tdata[36*8 +: 8] = 0;
                end
                
                rx_axis_tdata[16*8 +: 8] = header_length[15:8];
                rx_axis_tdata[17*8 +: 8] = header_length[7:0];
                // rx_axis_tdata[35*8 +: 8] = 0;
                rx_axis_tkeep = {64{1'b1}};
            end
            else begin
                rx_axis_tdata = c_counter + counter;  
                rx_axis_tkeep = {64{1'b1}};
            end
            if(c_counter == packet_len-1) begin
                rx_axis_tkeep = {64{1'b1}};
                rx_axis_tlast <= 1;
            end
        end
    end
         

end



always@(posedge clk) begin
    if(rst) begin
        counter <= 1;
        c_counter <= 0;
        cycle_counter <= 0;
        if_send <= 0;
    end
    else begin
        if_send <= ($urandom%1024 <= 400);
        ram_wr_cmd_valid_reg <= ram_wr_cmd_valid;
        if(start) begin
            cycle_counter <= cycle_counter + 1;
        end
        if(start && rx_axis_tready && rx_axis_tvalid) begin
            c_counter<= c_counter+1;
            if(c_counter == packet_len-1) begin
                c_counter <= 0;
                counter <= counter + 1;
            end
        end
    end
end

endmodule
