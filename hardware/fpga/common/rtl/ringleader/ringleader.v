`timescale 1ns / 1ps
`include "define.v"

module ringleader #
(
    // Width of AXI data bus in bits
    parameter AXI_DATA_WIDTH = 512,
    // Width of AXI wstrb (width of data bus in words)
    parameter AXI_STRB_WIDTH = (AXI_DATA_WIDTH/8),
    // Width of AXI ID signal
    parameter AXI_ID_WIDTH = 8,
    // Maximum AXI burst length to generate
    parameter AXI_MAX_BURST_LEN = 16,
    // Width of AXI stream interfaces in bits
    parameter AXIS_DATA_WIDTH = AXI_DATA_WIDTH,
    // Use AXI stream tkeep signal
    parameter AXIS_KEEP_ENABLE = (AXIS_DATA_WIDTH>8),
    // AXI stream tkeep signal width (words per cycle)
    parameter AXIS_KEEP_WIDTH = (AXIS_DATA_WIDTH/8),
    // Use AXI stream tlast signal
    parameter AXIS_LAST_ENABLE = 1,
    // Propagate AXI stream tid signal
    parameter AXIS_ID_ENABLE = 0,
    // AXI stream tid signal width
    parameter AXIS_ID_WIDTH = 8,
    // Propagate AXI stream tdest signal
    parameter AXIS_DEST_ENABLE = 0,
    // AXI stream tdest signal width
    parameter AXIS_DEST_WIDTH = 8,
    // Propagate AXI stream tuser signal
    parameter AXIS_USER_ENABLE = 0,
    // AXI stream tuser signal width
    parameter AXIS_USER_WIDTH = 1,
    // Width of control register interface address in bits
    parameter REG_ADDR_WIDTH = 16,
    // Width of control register interface data in bits
    parameter REG_DATA_WIDTH = 32,
    // Width of length field
    parameter LEN_WIDTH = 16,
    // Request tag field width
    parameter REQ_TAG_WIDTH = 8,
    // DMA RAM segment count
    parameter SEG_COUNT = 2,
    // DMA RAM segment data width
    parameter SEG_DATA_WIDTH = 64,
    // DMA RAM segment address width
    parameter SEG_ADDR_WIDTH = 8,
    // DMA RAM segment byte enable width
    parameter SEG_BE_WIDTH = SEG_DATA_WIDTH/8,
    // DMA RAM address width
    parameter RAM_ADDR_WIDTH = SEG_ADDR_WIDTH+$clog2(SEG_COUNT)+$clog2(SEG_BE_WIDTH),
    // DMA client length field width
    parameter DMA_CLIENT_LEN_WIDTH = 20,
    // DMA client tag field width
    parameter DMA_CLIENT_TAG_WIDTH = 8,
    parameter QUEUE_INDEX_WIDTH = 6,
    parameter QUEUE_PTR_WIDTH = 16,
    // DMA RX RAM size
    parameter RX_RAM_SIZE = 2**(RAM_ADDR_WIDTH+1),
    // Enable RX hashing
    parameter RX_HASH_ENABLE = 1,
    // Enable RX checksum offload
    parameter RX_CHECKSUM_ENABLE = 1

)
(
    input  wire                       clk,
    input  wire                       rst,
    /*
     * Control register interface
     */
    input  wire [REG_ADDR_WIDTH-1:0]            ctrl_reg_wr_addr,
    input  wire [REG_DATA_WIDTH-1:0]            ctrl_reg_wr_data,
    input  wire                                 ctrl_reg_wr_en,
    output reg                                  ctrl_reg_wr_ack,
    input  wire [REG_ADDR_WIDTH-1:0]            ctrl_reg_rd_addr,
    input  wire                                 ctrl_reg_rd_en,
    output reg [REG_DATA_WIDTH-1:0]             ctrl_reg_rd_data,
    output reg                                  ctrl_reg_rd_ack,


    /*
    * Receive data from the wire
    */
    input  wire [AXIS_DATA_WIDTH-1:0]           s_rx_axis_tdata,
    input  wire [AXIS_KEEP_WIDTH-1:0]           s_rx_axis_tkeep,
    input  wire                                 s_rx_axis_tvalid,
    output wire                                 s_rx_axis_tready,
    input  wire                                 s_rx_axis_tlast,
    input  wire                                 s_rx_axis_tuser,


    /*
     * RX scheduled desc to RX engine (queue index + mem addr)
     */
    output  wire [QUEUE_INDEX_WIDTH-1:0]     m_axis_rx_req_queue,
    output  wire [`APP_ID_WIDTH-1:0]         m_axis_rx_req_app_id,
    output  wire [REQ_TAG_WIDTH-1:0]         m_axis_rx_req_tag,
    output  wire                             m_axis_rx_req_valid,
    output  wire [RAM_ADDR_WIDTH-1:0]        m_axis_rx_req_ram_addr,
    output  wire [DMA_CLIENT_LEN_WIDTH-1:0]  m_axis_rx_req_len,
    output  wire [15:0]                      m_axis_rx_csum,
    output  wire [31:0]                      m_axis_rx_hash,
    input wire                               m_axis_rx_req_ready,


    // // Load feedback interface from queue manager
    input wire [QUEUE_INDEX_WIDTH-1:0]       s_axis_cpu_msg_queue,
    input wire                               s_axis_cpu_msg_valid,
    input wire [`CPU_MSG_WIDTH-1:0]          s_axis_cpu_msg,
    
    /*
     * Free memory address from RX buffer
     */
    output  wire                              free_mem_ready,
    input wire                                free_mem_req,
    input wire [LEN_WIDTH - 1 : 0]            free_mem_size,
    input wire [RAM_ADDR_WIDTH -1 : 0]           free_mem_addr,

    /*
     * RAM wr interface
     */
    output  wire [SEG_COUNT*SEG_BE_WIDTH-1:0]    ram_wr_cmd_be,
    output  wire [SEG_COUNT*SEG_ADDR_WIDTH-1:0]  ram_wr_cmd_addr,
    output  wire [SEG_COUNT*SEG_DATA_WIDTH-1:0]  ram_wr_cmd_data,
    output  wire [SEG_COUNT-1:0]                 ram_wr_cmd_valid,
    input wire [SEG_COUNT-1:0]                   ram_wr_cmd_ready,
    input wire [SEG_COUNT-1:0]                   ram_wr_done

);

wire                              free_mem_ready_1;
wire                              free_mem_req_1;
wire [LEN_WIDTH - 1 : 0]          free_mem_size_1;
wire [RAM_ADDR_WIDTH -1 : 0]      free_mem_addr_1;


wire                              free_mem_ready_2;
wire                              free_mem_req_2;
wire [LEN_WIDTH - 1 : 0]          free_mem_size_2;
wire [RAM_ADDR_WIDTH -1 : 0]      free_mem_addr_2;


wire                              free_mem_ready_select;
reg                              free_mem_req_select;
reg [LEN_WIDTH - 1 : 0]          free_mem_size_select;
reg [RAM_ADDR_WIDTH -1 : 0]      free_mem_addr_select;


// we do not need ready signal
always @(*) begin
    free_mem_req_select = 0;
    free_mem_size_select = 0;
    free_mem_addr_select = 0;

    if(free_mem_req_2) begin
        free_mem_req_select = free_mem_req_2;
        free_mem_size_select = free_mem_size_2;
        free_mem_addr_select = free_mem_addr_2;
    end
    else if(free_mem_req_1)  begin
        free_mem_req_select = free_mem_req_1;
        free_mem_size_select = free_mem_size_1;
        free_mem_addr_select = free_mem_addr_1;
    end
end

localparam APP_COUNT = (2**`APP_ID_WIDTH);

reg [QUEUE_INDEX_WIDTH-1:0] rss_mask_kernel_reg = 0;
reg [QUEUE_INDEX_WIDTH-1:0] rss_mask_user_reg = 0;

reg [QUEUE_INDEX_WIDTH-1:0] kernel_queue_offset_reg = 0;
reg [QUEUE_INDEX_WIDTH-1:0] user_queue_offset_reg = 0;

reg [QUEUE_PTR_WIDTH-1:0] dispatch_policy_reg = 0;
reg [QUEUE_PTR_WIDTH-1:0] host_queue_bound_reg = 0;

reg [31:0] user_space_ip_reg = 0;

reg [`APP_MSG_WIDTH-1:0] app_config_msg = 0;
reg [`APP_MSG_APP_ID_SIZE-1:0]      app_config_msg_app_id;
reg [`APP_MSG_APP_PORT_SIZE-1:0]   app_config_msg_port;
reg [`APP_MSG_APP_PRIO_SIZE-1:0]    app_config_msg_app_prio;
reg                              config_mat;


reg                              reset_monitor;
reg                              config_monitor;
reg [`APP_MSG_APP_ID_SIZE-1:0]   config_monitor_app_id;
reg [4:0]                        config_scale_down_epoch_log;
reg [4:0]                        config_cong_dectect_epoch_log;
reg [3:0]                        config_scale_down_thresh;


wire                             in_nic_msg_valid;
wire [`NIC_MSG_WIDTH - 1 : 0]    in_nic_msg;
wire                             in_nic_msg_ready;

wire                             out_nic_msg_valid;
wire [`NIC_MSG_WIDTH - 1 : 0]    out_nic_msg;
wire                             out_nic_msg_ready;

wire                            arm_cong_monitor;
wire                            arm_scale_down_monitor;
wire [`APP_ID_WIDTH-1:0]        arm_monitor_app_id;

reg                     app_config_msg_en = 0;

wire [APP_COUNT-1:0] app_eligbility_mask;

// control block
always @(posedge clk) begin
    ctrl_reg_wr_ack <= 1'b0;
    ctrl_reg_rd_data <= {REG_DATA_WIDTH{1'b0}};
    ctrl_reg_rd_ack <= 1'b0;
    app_config_msg_en <= 1'b0;
    app_config_msg <= {`APP_MSG_WIDTH{1'b0}};

    if (ctrl_reg_wr_en && !ctrl_reg_wr_ack) begin
        // write operation
        ctrl_reg_wr_ack <= 1'b1;
        case ({ctrl_reg_wr_addr >> 2, 2'b00})
            16'h0080: begin
                app_config_msg <= ctrl_reg_wr_data;
                app_config_msg_en <= 1'b1;
            end
            16'h0084: kernel_queue_offset_reg <= ctrl_reg_wr_data; // queue index offset for kernel space

            16'h0088: rss_mask_user_reg <= ctrl_reg_wr_data; // RSS mask for user space
            16'h008C: user_queue_offset_reg <= ctrl_reg_wr_data; // queue index offset for user space

            16'h0090: user_space_ip_reg <= ctrl_reg_wr_data; // queue index offset for user space
            16'h0094: dispatch_policy_reg <= ctrl_reg_wr_data; // queue index offset for user space
            16'h0098: host_queue_bound_reg <= ctrl_reg_wr_data; // queue index offset for user space
            default: ctrl_reg_wr_ack <= 1'b0;
        endcase
    end

    if (ctrl_reg_rd_en && !ctrl_reg_rd_ack) begin
        // read operation
        ctrl_reg_rd_ack <= 1'b1;
        case ({ctrl_reg_rd_addr >> 2, 2'b00})
            16'h0080: ctrl_reg_rd_data <= rss_mask_kernel_reg;
            16'h0084: ctrl_reg_rd_data <= kernel_queue_offset_reg;
            16'h0088: ctrl_reg_rd_data <= rss_mask_user_reg;
            16'h008C: ctrl_reg_rd_data <= user_queue_offset_reg;
            16'h0090: ctrl_reg_rd_data <= user_space_ip_reg;
            16'h0094: ctrl_reg_rd_data <= dispatch_policy_reg;
            16'h0098: ctrl_reg_rd_data <= host_queue_bound_reg;
            default: ctrl_reg_rd_ack <= 1'b0;
        endcase
    end

    if (rst) begin
        ctrl_reg_wr_ack <= 1'b0;
        ctrl_reg_rd_ack <= 1'b0;

        rss_mask_kernel_reg <= 0;
        kernel_queue_offset_reg <= 0;
        rss_mask_user_reg <= 0;
        user_queue_offset_reg <= 0;
        user_space_ip_reg <= 0;
    end
end


always @(*) begin
    app_config_msg_app_id = 0;
    app_config_msg_port = 0;
    app_config_msg_app_prio = 0;

    reset_monitor = 0;
    config_monitor = 0;
    config_mat = 0;
    config_scale_down_epoch_log = 0;
    config_cong_dectect_epoch_log = 0;

    if( app_config_msg_en && app_config_msg[`APP_MSG_TYPE_OF +: `APP_MSG_TYPE_SIZE] == `APP_MSG_CONIFG_APP) begin
        config_mat = 1;
        app_config_msg_app_id = app_config_msg[`APP_MSG_APP_ID_OF +: `APP_MSG_APP_ID_SIZE];
        app_config_msg_port = app_config_msg[`APP_MSG_APP_PORT_OF +: `APP_MSG_APP_PORT_SIZE];
        app_config_msg_app_prio = app_config_msg[`APP_MSG_APP_PRIO_OF +: `APP_MSG_APP_PRIO_SIZE];
    end
    else if(app_config_msg_en && app_config_msg[`APP_MSG_TYPE_OF +: `APP_MSG_TYPE_SIZE] == `APP_MSG_CONFIG_MONITOR) begin
        config_monitor = 1;
        config_monitor_app_id = app_config_msg[`APP_MSG_APP_ID_OF +: `APP_MSG_APP_ID_SIZE];

        config_scale_down_epoch_log = app_config_msg[`APP_MSG_APP_SCALE_DOWN_EPOCH_OF +: `APP_MSG_APP_SCALE_DOWN_EPOCH_SIZE];
        config_scale_down_thresh = app_config_msg[`APP_MSG_APP_SCALE_DOWN_THRESH_OF +: `APP_MSG_APP_SCALE_DOWN_THRESH_SIZE];
        config_cong_dectect_epoch_log = app_config_msg[`APP_MSG_APP_CONG_EPOCH_OF +: `APP_MSG_APP_CONG_EPOCH_SIZE];
    end
    else if(app_config_msg_en && app_config_msg[`APP_MSG_TYPE_OF +: `APP_MSG_TYPE_SIZE] == `APP_MSG_RESET_MONITOR) begin
        reset_monitor = 1;
    end

end

localparam CELL_NUM = RX_RAM_SIZE/`RL_CELL_SIZE;
localparam CELL_ID_WIDTH = $clog2(CELL_NUM) + 1;

reg [`RL_DESC_TS_SIZE-1:0] timestamp;
always @(posedge clk) begin
    if(rst) begin
        timestamp <= 0;
    end
    else begin
        timestamp <= timestamp +1;
    end
end


// wire for allocate memory  
wire                             alloc_mem_req;
wire [LEN_WIDTH - 1 : 0]         alloc_mem_size;
wire [CELL_ID_WIDTH -1 : 0]      alloc_cell_id;
wire                             alloc_mem_success;
wire                             alloc_mem_intense;

// // wire for free memory 
// wire                       free_mem_ready;
// wire                       free_mem_req;
// wire [LEN_WIDTH - 1 : 0]         free_mem_size; 
wire [CELL_ID_WIDTH - 1 : 0]     free_cell_id;
assign free_cell_id = free_mem_addr / `RL_CELL_SIZE;

wire [CELL_ID_WIDTH - 1 : 0]     free_cell_id_select;
assign free_cell_id_select = free_mem_addr_select / `RL_CELL_SIZE;

rand_mem_alloc #(
    .CELL_NUM(CELL_NUM),
    .CELL_ID_WIDTH(CELL_ID_WIDTH),
    .LEN_WIDTH(LEN_WIDTH)
)
rand_mem_alloc_inst (
    .clk(clk),
    .rst(rst),

    .alloc_mem_req(alloc_mem_req),
    .alloc_mem_size(alloc_mem_size),
    .alloc_cell_id(alloc_cell_id),
    .alloc_mem_success(alloc_mem_success),
    .alloc_mem_intense(alloc_mem_intense),

    .free_mem_req({free_mem_req, free_mem_req_select}),
    .free_mem_ready({free_mem_ready, free_mem_ready_select}),
    .free_mem_size({free_mem_size, free_mem_size_select}),
    .free_cell_id({free_cell_id, free_cell_id_select})
);




// rx data to axis_sink

wire [AXIS_DATA_WIDTH-1:0]           sink_rx_axis_tdata;
wire [AXIS_KEEP_WIDTH-1:0]           sink_rx_axis_tkeep;
wire                                 sink_rx_axis_tvalid;
wire                                 sink_rx_axis_tready;
wire                                 sink_rx_axis_tlast;
wire                                 sink_rx_axis_tuser;

// packet desc to hw scheduler
wire [`RL_DESC_WIDTH-1:0]             sche_packet_desc;
wire                                  sche_packet_desc_valid;
wire                                  sche_packet_desc_ready;
wire [`RL_DESC_WIDTH-1:0]             gen_packet_desc;
wire                                  gen_packet_desc_valid;
wire                                  gen_packet_desc_ready;


wire                                  sche_pifo_valid;
wire [`RL_DESC_APP_ID_SIZE-1:0]       sche_pifo_prio;
wire [`RL_DESC_APP_ID_SIZE-1:0]       sche_pifo_data;
wire                                  sche_pifo_ready;
wire [`RL_DESC_APP_ID_SIZE-1:0]       sche_pifo_empty_data;
wire                                  sche_pifo_empty;



wire [`RL_DESC_WIDTH-1:0]               qm_packet_desc;
wire                                    qm_packet_desc_valid;
wire                                    qm_packet_desc_req;
wire [`RL_DESC_APP_ID_SIZE-1:0]         qm_packet_desc_app_id;

// rx ram write desc to axis_sink
wire [RAM_ADDR_WIDTH-1:0]       sink_rx_desc_addr;
wire [DMA_CLIENT_LEN_WIDTH-1:0] sink_rx_desc_len;
wire [DMA_CLIENT_TAG_WIDTH-1:0] sink_rx_desc_tag;
wire                            sink_rx_desc_valid;
wire                            sink_rx_desc_ready;

wire [DMA_CLIENT_LEN_WIDTH-1:0] sink_rx_desc_status_len;
wire [DMA_CLIENT_TAG_WIDTH-1:0] sink_rx_desc_status_tag;
wire                            sink_rx_desc_status_user;
wire [3:0]                      sink_rx_desc_status_error;
wire                            sink_rx_desc_status_valid;



desc_gen #(
    .AXIS_DATA_WIDTH(AXIS_DATA_WIDTH),
    .AXIS_KEEP_WIDTH(AXIS_KEEP_WIDTH),
    .AXIS_LAST_ENABLE(AXIS_LAST_ENABLE),
    .AXIS_ID_ENABLE(AXIS_ID_ENABLE),
    .AXIS_DEST_ENABLE(AXIS_DEST_ENABLE),
    .AXIS_USER_ENABLE(AXIS_USER_ENABLE),
    .LEN_WIDTH(LEN_WIDTH),
    .CELL_ID_WIDTH(CELL_ID_WIDTH),
    .RAM_ADDR_WIDTH(RAM_ADDR_WIDTH),
    .DMA_CLIENT_LEN_WIDTH(DMA_CLIENT_LEN_WIDTH),
    .DMA_CLIENT_TAG_WIDTH(DMA_CLIENT_TAG_WIDTH),
    .RX_HASH_ENABLE(RX_HASH_ENABLE),
    .RX_CHECKSUM_ENABLE(RX_CHECKSUM_ENABLE)
)
desc_gen_inst(

    .clk(clk),
    .rst(rst),

    /*
    * Receive data from the wire
    */
    .s_rx_axis_tdata(s_rx_axis_tdata),
    .s_rx_axis_tkeep(s_rx_axis_tkeep),
    .s_rx_axis_tvalid(s_rx_axis_tvalid),
    .s_rx_axis_tready(s_rx_axis_tready),
    .s_rx_axis_tlast(s_rx_axis_tlast),
    .s_rx_axis_tuser(s_rx_axis_tuser),


    /*
    * Send packet data to the packet scheduler
    */
    .m_rx_axis_tdata(sink_rx_axis_tdata),
    .m_rx_axis_tkeep(sink_rx_axis_tkeep),
    .m_rx_axis_tvalid(sink_rx_axis_tvalid),
    .m_rx_axis_tready(sink_rx_axis_tready),
    .m_rx_axis_tlast(sink_rx_axis_tlast),
    .m_rx_axis_tuser(sink_rx_axis_tuser),


    /*
    * Memory allocator assign memory address for each packet
    * The memory address is contained in the packet descriptor
    */
    .alloc_mem_req(alloc_mem_req),
    .alloc_mem_size(alloc_mem_size),
    .alloc_cell_id(alloc_cell_id),
    .alloc_mem_success(alloc_mem_success),
    .alloc_mem_intense(alloc_mem_intense),

    /*
    * Send packet descriptor to the packet scheduler
    */
    .m_packet_desc(gen_packet_desc),
    .m_packet_desc_valid(gen_packet_desc_valid),
    .m_packet_desc_ready(gen_packet_desc_ready),


    /*
     * Receive descriptor output
     */
    .m_axis_rx_desc_addr(sink_rx_desc_addr),
    .m_axis_rx_desc_len(sink_rx_desc_len),
    .m_axis_rx_desc_tag(sink_rx_desc_tag),
    .m_axis_rx_desc_valid(sink_rx_desc_valid),
    .m_axis_rx_desc_ready(sink_rx_desc_ready),

    /*
     * Receive descriptor status input
     */
    .s_axis_rx_desc_status_len(sink_rx_desc_status_len),
    .s_axis_rx_desc_status_tag(sink_rx_desc_status_tag),
    .s_axis_rx_desc_status_user(sink_rx_desc_status_user),
    .s_axis_rx_desc_status_error(sink_rx_desc_status_error),
    .s_axis_rx_desc_status_valid(sink_rx_desc_status_valid),

    .timestamp(timestamp),
    .app_config_msg_app_id(app_config_msg_app_id),
    .app_config_msg_port(app_config_msg_port),
    .app_config_msg_app_prio(app_config_msg_app_prio),
    .app_config_msg_en(config_mat),
    .user_space_ip(user_space_ip_reg)
);

// assign sche_packet_desc = gen_packet_desc;
// assign sche_packet_desc_valid = gen_packet_desc_valid;
// assign gen_packet_desc_ready = sche_packet_desc_ready;


axis_fifo #(
    .DEPTH(16),
    .DATA_WIDTH(`NIC_MSG_WIDTH),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0)
)
global_nic_msg_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(in_nic_msg),
    .s_axis_tvalid(in_nic_msg_valid),
    .s_axis_tready(in_nic_msg_ready),

    // AXI output
    .m_axis_tdata(out_nic_msg),
    .m_axis_tvalid(out_nic_msg_valid),
    .m_axis_tready(out_nic_msg_ready)
);


// queue manager
pifo_queue_manager #(
    .RAM_ADDR_WIDTH(RAM_ADDR_WIDTH),
    .CELL_NUM(CELL_NUM),
    .QUEUE_NUMBER_WIDTH(`RL_DESC_APP_ID_SIZE),
    .PER_QUEUE_RAM_SIZE(256)
)
pifo_queue_manager_inst(
    .clk(clk),
    .rst(rst),

    .s_packet_desc(gen_packet_desc),
    .s_packet_desc_valid(gen_packet_desc_valid),
    .s_packet_desc_app_id(gen_packet_desc_valid? gen_packet_desc[`RL_DESC_APP_ID_OF +: `RL_DESC_APP_ID_SIZE] : 0),
    .s_packet_desc_ready(gen_packet_desc_ready),

    .m_packet_desc_req(qm_packet_desc_req),
    .m_packet_desc_app_id(qm_packet_desc_app_id),
    .m_packet_desc(qm_packet_desc),
    .m_packet_desc_valid(qm_packet_desc_valid),


    .m_pifo_valid(sche_pifo_valid),
    .m_pifo_prio(sche_pifo_prio), 
    .m_pifo_data(sche_pifo_data),
    .m_pifo_ready(sche_pifo_ready),
    .m_pifo_empty(sche_pifo_empty),
    .m_pifo_empty_data(sche_pifo_empty_data),

    .free_mem_ready(free_mem_ready_2),
    .free_mem_req(free_mem_req_2),
    .free_mem_size(free_mem_size_2),
    .free_mem_addr(free_mem_addr_2),


    .reset_monitor(reset_monitor),
    .config_monitor(config_monitor),
    .config_monitor_app_id(config_monitor_app_id),
    .config_scale_down_epoch_log(config_scale_down_epoch_log),
    .config_cong_dectect_epoch_log(config_cong_dectect_epoch_log),
    .config_scale_down_thresh(config_scale_down_thresh),

    .arm_cong_monitor(arm_cong_monitor),
    .arm_scale_down_monitor(arm_scale_down_monitor),
    .arm_monitor_app_id(arm_monitor_app_id),

    .nic_msg_valid(in_nic_msg_valid),
    .nic_msg(in_nic_msg),
    .nic_msg_ready(in_nic_msg_ready)

);

// determines the order of the scheduled descriptor
 desc_sche_pifo #(
     .APP_ELI_MASK_WIDTH(APP_COUNT)
 )
 desc_sche_inst(
     .clk(clk),
     .rst(rst),

     .m_packet_desc(sche_packet_desc),
     .m_packet_desc_valid(sche_packet_desc_valid),
     .m_packet_desc_ready(sche_packet_desc_ready),
    

     .qm_packet_desc(qm_packet_desc),
     .qm_packet_desc_valid(qm_packet_desc_valid),
     .qm_packet_desc_req(qm_packet_desc_req),
     .qm_packet_desc_app_id(qm_packet_desc_app_id),

     .s_pifo_valid(sche_pifo_valid),
     .s_pifo_prio(sche_pifo_prio), 
     .s_pifo_data(sche_pifo_data),
     .s_pifo_ready(sche_pifo_ready),
     .s_pifo_empty(sche_pifo_empty),
     .s_pifo_empty_data(sche_pifo_empty_data),
     

     .s_app_mask(app_eligbility_mask)

 );

// uncomment this for simulator
// desc_sche #(
//    .APP_ELI_MASK_WIDTH(APP_COUNT)
// )
// desc_sche_inst(
//    .clk(clk),
//    .rst(rst),

//    .m_packet_desc(sche_packet_desc),
//    .m_packet_desc_valid(sche_packet_desc_valid),
//    .m_packet_desc_ready(sche_packet_desc_ready),
    

//    .qm_packet_desc(qm_packet_desc),
//    .qm_packet_desc_valid(qm_packet_desc_valid),
//    .qm_packet_desc_req(qm_packet_desc_req),
//    .qm_packet_desc_app_id(qm_packet_desc_app_id),

//    .s_pifo_valid(sche_pifo_valid),
//    .s_pifo_prio(sche_pifo_prio), 
//    .s_pifo_data(sche_pifo_data),
//    .s_pifo_ready(sche_pifo_ready),

//    .s_app_mask(app_eligbility_mask)

// );
// assign sche_pifo_empty = 0;

// determines the load balancing algorithm

desc_dispatch#(
    .AXI_DATA_WIDTH(AXI_DATA_WIDTH),
    .DMA_CLIENT_LEN_WIDTH(DMA_CLIENT_LEN_WIDTH),
    .REQ_TAG_WIDTH(REQ_TAG_WIDTH),
    .RAM_ADDR_WIDTH(RAM_ADDR_WIDTH),
    .QUEUE_INDEX_WIDTH(QUEUE_INDEX_WIDTH),
    .QUEUE_PTR_WIDTH(QUEUE_PTR_WIDTH),
    .REDUCE_TREE_INDEX_WIDTH(`CORE_COUNT_WIDTH),
    .REDUCE_TREE_PTR_WIDTH(QUEUE_PTR_WIDTH),
    .APP_COUNT(APP_COUNT)
)
desc_dispatch_inst(
    .clk(clk),
    .rst(rst),

    .s_packet_desc(sche_packet_desc_valid? sche_packet_desc : 0),
    .s_packet_desc_valid(sche_packet_desc_valid),
    .s_packet_desc_ready(sche_packet_desc_ready),

    .m_axis_rx_req_queue(m_axis_rx_req_queue),
    .m_axis_rx_req_app_id(m_axis_rx_req_app_id),
    .m_axis_rx_req_ram_addr(m_axis_rx_req_ram_addr),
    .m_axis_rx_req_len(m_axis_rx_req_len),
    .m_axis_rx_csum(m_axis_rx_csum),
    .m_axis_rx_hash(m_axis_rx_hash),
    .m_axis_rx_req_tag(m_axis_rx_req_tag),
    .m_packet_desc_valid(m_axis_rx_req_valid),
    .m_packet_desc_ready(m_axis_rx_req_ready),

    .s_axis_cpu_msg_queue(s_axis_cpu_msg_queue),
    .s_axis_cpu_msg_valid(s_axis_cpu_msg_valid),
    .s_axis_cpu_msg(s_axis_cpu_msg),

    .rank_upbound(host_queue_bound_reg),
    .sche_policy(dispatch_policy_reg),
    .rss_mask_kernel_reg(rss_mask_kernel_reg),
    .rss_mask_user_reg(rss_mask_user_reg),
    .kernel_queue_offset_reg(kernel_queue_offset_reg),
    .user_queue_offset_reg(user_queue_offset_reg),

    .free_mem_ready(free_mem_ready_1),
    .free_mem_req(free_mem_req_1),
    .free_mem_size(free_mem_size_1),
    .free_mem_addr(free_mem_addr_1),

    .nic_msg_valid(out_nic_msg_valid),
    .nic_msg(out_nic_msg),
    .nic_msg_ready(out_nic_msg_ready),

    .arm_cong_monitor(arm_cong_monitor),
    .arm_scale_down_monitor(arm_scale_down_monitor),
    .arm_monitor_app_id(arm_monitor_app_id),

    .m_app_mask(app_eligbility_mask)
);


// Change RX Data's AXIS interface to RAM interface
dma_client_axis_sink #(
    .SEG_COUNT(SEG_COUNT),
    .SEG_DATA_WIDTH(SEG_DATA_WIDTH),
    .SEG_ADDR_WIDTH(SEG_ADDR_WIDTH),
    .SEG_BE_WIDTH(SEG_BE_WIDTH),
    .RAM_ADDR_WIDTH(RAM_ADDR_WIDTH),
    .AXIS_DATA_WIDTH(AXIS_DATA_WIDTH),
    .AXIS_KEEP_ENABLE(AXIS_KEEP_WIDTH > 1),
    .AXIS_KEEP_WIDTH(AXIS_KEEP_WIDTH),
    .AXIS_LAST_ENABLE(1),
    .AXIS_ID_ENABLE(0),
    .AXIS_DEST_ENABLE(0),
    .AXIS_USER_ENABLE(1),
    .AXIS_USER_WIDTH(1),
    .LEN_WIDTH(DMA_CLIENT_LEN_WIDTH),
    .TAG_WIDTH(DMA_CLIENT_TAG_WIDTH)
)
dma_client_axis_sink_inst (
    .clk(clk),
    .rst(rst),

    /*
     * DMA write descriptor input
     */
    .s_axis_write_desc_ram_addr(sink_rx_desc_addr),
    .s_axis_write_desc_len(sink_rx_desc_len),
    .s_axis_write_desc_tag(sink_rx_desc_tag),
    .s_axis_write_desc_valid(sink_rx_desc_valid),
    .s_axis_write_desc_ready(sink_rx_desc_ready),

    /*
     * DMA write descriptor status output
     */
    .m_axis_write_desc_status_len(sink_rx_desc_status_len),
    .m_axis_write_desc_status_tag(sink_rx_desc_status_tag),
    .m_axis_write_desc_status_id(),
    .m_axis_write_desc_status_dest(),
    .m_axis_write_desc_status_user(sink_rx_desc_status_user),
    .m_axis_write_desc_status_error(sink_rx_desc_status_error),
    .m_axis_write_desc_status_valid(sink_rx_desc_status_valid),

    /*
     * AXI stream write data input
     */
    .s_axis_write_data_tdata(sink_rx_axis_tdata),
    .s_axis_write_data_tkeep(sink_rx_axis_tkeep),
    .s_axis_write_data_tvalid(sink_rx_axis_tvalid),
    .s_axis_write_data_tready(sink_rx_axis_tready),
    .s_axis_write_data_tlast(sink_rx_axis_tlast),
    .s_axis_write_data_tid(0),
    .s_axis_write_data_tdest(0),
    .s_axis_write_data_tuser(sink_rx_axis_tuser),

    /*
     * RAM interface
     */
    .ram_wr_cmd_be(ram_wr_cmd_be),
    .ram_wr_cmd_addr(ram_wr_cmd_addr),
    .ram_wr_cmd_data(ram_wr_cmd_data),
    .ram_wr_cmd_valid(ram_wr_cmd_valid),
    .ram_wr_cmd_ready(ram_wr_cmd_ready),
    .ram_wr_done(ram_wr_done),

    /*
     * Configuration
     */
    .enable(1'b1),
    .abort(1'b0)
);

// ila_0 param_perf_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(app_config_msg_en), // input wire [0:0] probe0  
// 	.probe1({app_config_msg_en, app_config_msg[`APP_MSG_TYPE_OF +: `APP_MSG_TYPE_SIZE], config_mat, config_monitor, reset_monitor})
// );

endmodule