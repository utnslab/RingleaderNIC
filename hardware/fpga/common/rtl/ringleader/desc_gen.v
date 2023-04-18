
`timescale 1ns / 1ps
`include "define.v"

module desc_gen #
(
    // Width of AXI stream interfaces in bits
    parameter AXIS_DATA_WIDTH = 512,
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
    
    // parameter AXI_ADDR_WIDTH = 16,
    parameter LEN_WIDTH = 16,

    parameter CELL_ID_WIDTH = 16,

    // DMA RAM address width
    parameter RAM_ADDR_WIDTH = 16,
    // DMA client length field width
    parameter DMA_CLIENT_LEN_WIDTH = 20,
    // DMA client tag field width
    parameter DMA_CLIENT_TAG_WIDTH = 8,

    parameter RX_HASH_ENABLE = 1,
    // Enable RX checksum offload
    parameter RX_CHECKSUM_ENABLE = 1

)
(
    input  wire                       clk,
    input  wire                       rst,
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
    * Send data output to the axis_sink
    */
    output  reg [AXIS_DATA_WIDTH-1:0]          m_rx_axis_tdata,
    output  reg [AXIS_KEEP_WIDTH-1:0]          m_rx_axis_tkeep,
    output  reg                                m_rx_axis_tvalid,
    input   wire                               m_rx_axis_tready,
    output  reg                                m_rx_axis_tlast,
    output  reg                                m_rx_axis_tuser,

    /*memory address form the memory allocator*/
    output reg                                  alloc_mem_req,
    output reg  [LEN_WIDTH - 1 : 0]             alloc_mem_size,
    input  wire [CELL_ID_WIDTH - 1 : 0]         alloc_cell_id,
    input  wire                                 alloc_mem_success,
    input  wire                                 alloc_mem_intense,

    /* output packet descriptor*/
    output wire [`RL_DESC_WIDTH-1:0]             m_packet_desc,
    output wire                                  m_packet_desc_valid,
    input  wire                                 m_packet_desc_ready,

    /*
     * Receive descriptor output, send choosed desc to dma_client_axis_sink_inst, which
     * writes data to DMA ram accordingly.
     */
    output reg [RAM_ADDR_WIDTH-1:0]        m_axis_rx_desc_addr,
    output reg [DMA_CLIENT_LEN_WIDTH-1:0]  m_axis_rx_desc_len,
    output reg [DMA_CLIENT_TAG_WIDTH-1:0]  m_axis_rx_desc_tag,
    output reg                             m_axis_rx_desc_valid,
    input  wire                             m_axis_rx_desc_ready,

    /*
     * Receive descriptor status input
     */
    input  wire [DMA_CLIENT_LEN_WIDTH-1:0]  s_axis_rx_desc_status_len,
    input  wire [DMA_CLIENT_TAG_WIDTH-1:0]  s_axis_rx_desc_status_tag,
    input  wire                             s_axis_rx_desc_status_user,
    input  wire [3:0]                       s_axis_rx_desc_status_error,
    input  wire                             s_axis_rx_desc_status_valid,

    /*
     * Configuration
     */
    input   wire [`RL_DESC_TS_SIZE-1:0]    timestamp,
    input wire  [4*8-1:0]         user_space_ip,
    input wire [`APP_MSG_APP_ID_SIZE-1:0]    app_config_msg_app_id,
    input wire [`APP_MSG_APP_PORT_SIZE-1: 0] app_config_msg_port,
    input wire [`APP_MSG_APP_PRIO_SIZE-1:0]  app_config_msg_app_prio,
    input wire                     app_config_msg_en,
    input  wire [DMA_CLIENT_LEN_WIDTH-1:0]  mtu

);

parameter AXIS_KEEP_WIDTH_INT = AXIS_KEEP_ENABLE ? AXIS_KEEP_WIDTH : 1;
parameter OFFSET_WIDTH = AXIS_KEEP_WIDTH_INT > 1 ? $clog2(AXIS_KEEP_WIDTH_INT) : 1;
reg [LEN_WIDTH-1:0] length_reg = {LEN_WIDTH{1'b0}};
reg length_fifo_valid;
reg input_head;
reg [OFFSET_WIDTH:0] cycle_size;

integer i;
always @(*) begin
    cycle_size = AXIS_KEEP_WIDTH_INT;
    for (i = AXIS_KEEP_WIDTH_INT-1; i >= 0; i = i - 1) begin
        if (~(s_rx_axis_tkeep & {AXIS_KEEP_WIDTH_INT{1'b1}}) & (1 << i)) begin
            cycle_size = i;
        end
    end
end

always @(posedge clk) begin
    if(rst) begin
      input_head <= 0;
      length_reg <= 0;
      length_fifo_valid <= 0;
    end
    else begin
      length_fifo_valid <= 0;

      if(s_rx_axis_tready && s_rx_axis_tvalid) begin
        if(input_head) begin
          length_reg <= cycle_size;
        end
        else begin
          length_reg <= length_reg + cycle_size;
        end
      end

      if(s_rx_axis_tready && s_rx_axis_tvalid && s_rx_axis_tlast) begin
        input_head <= 1;
        length_fifo_valid <= 1;
      end
      else if(s_rx_axis_tready && s_rx_axis_tvalid) begin
        input_head <= 0;
      end

    end
    
end


wire [LEN_WIDTH-1:0]                 fifo_length_data;
wire                                 fifo_length_valid;
reg                                  fifo_length_ready;

wire [AXIS_DATA_WIDTH-1:0]           fifo_rx_axis_tdata;
wire [AXIS_KEEP_WIDTH-1:0]           fifo_rx_axis_tkeep;
wire                                 fifo_rx_axis_tvalid;
reg                                  fifo_rx_axis_tready;
wire                                 fifo_rx_axis_tlast;
wire                                 fifo_rx_axis_tuser;

// RX hashing
wire [31:0]              rx_hash;
wire [3:0]               rx_hash_type;
wire                     rx_hash_valid;

wire [31:0]              rx_fifo_hash;
wire [3:0]               rx_fifo_hash_type;
wire                     rx_fifo_hash_valid;
reg                      rx_fifo_hash_ready;

// Checksums
wire [15:0]              rx_csum;
wire                     rx_csum_valid;

wire [15:0]              rx_fifo_csum;
wire                     rx_fifo_csum_valid;
reg                      rx_fifo_csum_ready;

axis_fifo #(
    .DEPTH(32 * AXIS_KEEP_WIDTH),
    .DATA_WIDTH(AXIS_DATA_WIDTH),
    .KEEP_ENABLE(1),
    .KEEP_WIDTH(AXIS_KEEP_WIDTH),
    .LAST_ENABLE(1),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(1),
    .USER_WIDTH(1),
    .FRAME_FIFO(0)
)
hash_csum_data_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(s_rx_axis_tdata),
    .s_axis_tkeep(s_rx_axis_tkeep),
    .s_axis_tvalid(s_rx_axis_tvalid),
    .s_axis_tready(s_rx_axis_tready),
    .s_axis_tlast(s_rx_axis_tlast),
    .s_axis_tuser(s_rx_axis_tuser),

    // AXI output
    .m_axis_tdata(fifo_rx_axis_tdata),
    .m_axis_tkeep(fifo_rx_axis_tkeep),
    .m_axis_tvalid(fifo_rx_axis_tvalid),
    .m_axis_tready(fifo_rx_axis_tready),
    .m_axis_tlast(fifo_rx_axis_tlast),
    .m_axis_tuser(fifo_rx_axis_tuser)
);

axis_fifo #(
    .DEPTH(32),
    .DATA_WIDTH(AXIS_DATA_WIDTH),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0)
)
pk_len_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(length_reg),
    .s_axis_tvalid(length_fifo_valid),
    .s_axis_tready(),

    // AXI output
    .m_axis_tdata(fifo_length_data),
    .m_axis_tvalid(fifo_length_valid),
    .m_axis_tready(fifo_length_ready)
);


if (RX_HASH_ENABLE) begin

    rx_hash #(
        .DATA_WIDTH(AXIS_DATA_WIDTH)
    )
    rx_hash_inst (
        .clk(clk),
        .rst(rst),
        .s_axis_tdata(s_rx_axis_tdata),
        .s_axis_tkeep(s_rx_axis_tkeep),
        .s_axis_tvalid(s_rx_axis_tvalid & s_rx_axis_tready),
        .s_axis_tlast(s_rx_axis_tlast),
        .hash_key(320'h6d5a56da255b0ec24167253d43a38fb0d0ca2bcbae7b30b477cb2da38030f20c6a42b73bbeac01fa),
        .m_axis_hash(rx_hash),
        .m_axis_hash_type(rx_hash_type),
        .m_axis_hash_valid(rx_hash_valid)
    );

    axis_fifo #(
        .DEPTH(32),
        .DATA_WIDTH(32+4),
        .KEEP_ENABLE(0),
        .LAST_ENABLE(0),
        .ID_ENABLE(0),
        .DEST_ENABLE(0),
        .USER_ENABLE(0),
        .FRAME_FIFO(0)
    )
    rx_hash_fifo (
        .clk(clk),
        .rst(rst),

        // AXI input
        .s_axis_tdata({rx_hash_type, rx_hash}),
        .s_axis_tvalid(rx_hash_valid),
        .s_axis_tready(),


        // AXI output
        .m_axis_tdata({rx_fifo_hash_type, rx_fifo_hash}),
        .m_axis_tvalid(rx_fifo_hash_valid),
        .m_axis_tready(rx_fifo_hash_ready)

    );

end else begin

    assign rx_fifo_hash = 32'd0;
    assign rx_fifo_type = 4'd0;
    assign rx_fifo_hash_valid = 1'b1;

end


if (RX_CHECKSUM_ENABLE) begin

    rx_checksum #(
        .DATA_WIDTH(AXIS_DATA_WIDTH)
    )
    rx_checksum_inst (
        .clk(clk),
        .rst(rst),
        .s_axis_tdata(s_rx_axis_tdata),
        .s_axis_tkeep(s_rx_axis_tkeep),
        .s_axis_tvalid(s_rx_axis_tvalid & s_rx_axis_tready),
        .s_axis_tlast(s_rx_axis_tlast),
        .m_axis_csum(rx_csum),
        .m_axis_csum_valid(rx_csum_valid)
    );

    axis_fifo #(
        .DEPTH(32),
        .DATA_WIDTH(16),
        .KEEP_ENABLE(0),
        .LAST_ENABLE(0),
        .ID_ENABLE(0),
        .DEST_ENABLE(0),
        .USER_ENABLE(0),
        .FRAME_FIFO(0)
    )
    rx_csum_fifo (
        .clk(clk),
        .rst(rst),

        // AXI input
        .s_axis_tdata(rx_csum),
        .s_axis_tvalid(rx_csum_valid),
        .s_axis_tready(),

        // AXI output
        .m_axis_tdata(rx_fifo_csum),
        .m_axis_tvalid(rx_fifo_csum_valid),
        .m_axis_tready(rx_fifo_csum_ready)
    );

end else begin

    assign rx_fifo_csum = 16'd0;
    assign rx_fifo_csum_valid = 1'b1;

end

reg                                          desc_next;
reg                                          parse_req;
wire [`RL_DESC_PRIO_SIZE -1 : 0]          desc_prio;
wire [`RL_DESC_LEN_SIZE - 1 : 0]          desc_pk_len;
wire [`RL_DESC_APP_ID_SIZE -1 : 0]          desc_app_id;

reg  [`RL_DESC_WIDTH-1:0]             o_fifo_packet_desc;
reg                                   o_fifo_packet_desc_valid;
wire                                 o_fifo_packet_desc_ready;

header_parser #
(
    .DATA_WIDTH(AXIS_DATA_WIDTH),
    .KEEP_WIDTH(AXIS_KEEP_WIDTH)
)
header_parser(
    .clk(clk),
    .rst(rst),
    .user_space_ip(user_space_ip),

    .s_axis_tdata(fifo_rx_axis_tdata),
    .s_axis_tkeep(fifo_rx_axis_tkeep),
    .s_axis_tvalid(parse_req),
    .s_axis_tlast(parse_req),    

    .app_config_msg_app_id(app_config_msg_app_id),
    .app_config_msg_port(app_config_msg_port),
    .app_config_msg_app_prio(app_config_msg_app_prio),
    .app_config_msg_en(app_config_msg_en),
    .m_desc_prio(desc_prio),
    .m_desc_pk_len(desc_pk_len),
    .m_desc_app_id(desc_app_id)
);


reg   [15: 0] cycle_counter;

reg [1:0] parser_state;
reg [4:0] next_dest_port;
reg if_drop;
localparam PK_HEAD_STATE = 0;
localparam PK_DATA_STATE = 1;

always @(posedge clk) begin
    if(rst) begin
        parser_state <= PK_HEAD_STATE;
        if_drop <= 0;
        next_dest_port <= 0;
    end
    else begin
        cycle_counter = cycle_counter + 1;
        if(parser_state == PK_HEAD_STATE) begin
            if(o_fifo_packet_desc_ready && o_fifo_packet_desc_valid ) begin
                parser_state <= PK_DATA_STATE;
            end
        end
        else if (parser_state == PK_DATA_STATE) begin
            if(fifo_rx_axis_tvalid && fifo_rx_axis_tready && fifo_rx_axis_tlast) begin
                parser_state <= PK_HEAD_STATE;
            end
        end
    end
end 


always @* begin
    m_rx_axis_tdata = 0;
    m_rx_axis_tvalid = 0;
    m_rx_axis_tkeep = 0;
    m_rx_axis_tuser = 0; 
    m_rx_axis_tlast = 0;

    o_fifo_packet_desc = 0;
    o_fifo_packet_desc_valid = 0;

    m_axis_rx_desc_addr = 0;
    m_axis_rx_desc_len = 0;
    m_axis_rx_desc_tag = 0;
    m_axis_rx_desc_valid = 0;

    fifo_rx_axis_tready = 0;

    rx_fifo_csum_ready = 0;
    rx_fifo_hash_ready = 0;
    fifo_length_ready = 0;
    
    alloc_mem_req = 0;
    alloc_mem_size = 0;
    parse_req = 0;

    if((parser_state == PK_HEAD_STATE) && fifo_rx_axis_tvalid && !alloc_mem_intense) begin // packet header
        parse_req = 1;

        if(m_axis_rx_desc_ready && o_fifo_packet_desc_ready && rx_fifo_hash_valid && rx_fifo_csum_valid && fifo_length_valid) begin // generate descriptor and ram write req
            alloc_mem_req = 1;
            alloc_mem_size = `RL_CELL_SIZE;
            o_fifo_packet_desc[`RL_DESC_LEN_OF   +: `RL_DESC_LEN_SIZE]  = fifo_length_data;
            o_fifo_packet_desc[`RL_DESC_CELL_ID_OF  +: `RL_DESC_CELL_ID_SIZE]  = alloc_cell_id;
            o_fifo_packet_desc[`RL_DESC_DROP_OF]                          = 0;
            o_fifo_packet_desc[`RL_DESC_PRIO_OF  +: `RL_DESC_PRIO_SIZE] =  desc_prio; // calculate priority, which is time stamp here
            o_fifo_packet_desc[`RL_DESC_APP_ID_OF +: `RL_DESC_APP_ID_SIZE] = desc_app_id;
            o_fifo_packet_desc[`RL_DESC_TS_OF +: `RL_DESC_TS_SIZE]= timestamp;

            o_fifo_packet_desc[`RL_DESC_CSUM_OF +: `RL_DESC_CSUM_SIZE]= rx_fifo_csum;
            o_fifo_packet_desc[`RL_DESC_HASH_OF +: `RL_DESC_HASH_SIZE]= rx_fifo_hash;

            // HASH and CSUM FIFO
            rx_fifo_hash_ready = 1;
            rx_fifo_csum_ready = 1;
            fifo_length_ready = 1;
        
            // RAM write descriptor  
            m_axis_rx_desc_addr = alloc_cell_id * `RL_CELL_SIZE;
            m_axis_rx_desc_len = `RL_CELL_SIZE;
            m_axis_rx_desc_tag = 0;
            m_axis_rx_desc_valid = 1;

            // packet descriptor to scheduler
            o_fifo_packet_desc_valid = 1;
                
        end
 
    end
    else if(parser_state ==PK_DATA_STATE) begin
        if(if_drop) begin
            fifo_rx_axis_tready = 1;
        end
        else begin
            m_rx_axis_tvalid = fifo_rx_axis_tvalid;
            m_rx_axis_tdata = fifo_rx_axis_tdata;
            m_rx_axis_tlast = fifo_rx_axis_tlast;
            m_rx_axis_tuser = 0;  

            m_rx_axis_tkeep = fifo_rx_axis_tkeep;
            fifo_rx_axis_tready = m_rx_axis_tready;
        end

    end
end

axis_fifo #(
    .DEPTH(4),
    .DATA_WIDTH(`RL_DESC_WIDTH),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0)
)
gen_out_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(o_fifo_packet_desc),
    .s_axis_tvalid(o_fifo_packet_desc_valid),
    .s_axis_tready(o_fifo_packet_desc_ready),

    // AXI output
    .m_axis_tdata(m_packet_desc),
    .m_axis_tvalid(m_packet_desc_valid),
    .m_axis_tready(m_packet_desc_ready)
);

endmodule
