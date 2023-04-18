`timescale 1ns / 1ps
`include "define.v"

module rl_mem_alloc #
(
    // Width of AXI address bus in bits
    // parameter AXI_ADDR_WIDTH = 16,
    parameter LEN_WIDTH = 16,
    parameter CELL_NUM = 64,
    // parameter CELL_SIZE = 512 * 3 * 8,  // in bit
    parameter DROP_THRESH = 2,   // if current free memoey smaller than 2 cell, then drop the pacekt in PIFO
    parameter CELL_ID_WIDTH = $clog2(CELL_NUM)
)
(
    input  wire                             clk,
    input  wire                             rst,

    input  wire                             alloc_mem_req,
    input  wire [LEN_WIDTH - 1 : 0]         alloc_mem_size,
    output wire [CELL_ID_WIDTH - 1 : 0]     alloc_cell_id,
    output wire                              alloc_mem_success,
    output wire                              alloc_mem_intense,

    input  wire                       free_mem_req,
    output wire                       free_mem_ready,
    input  wire [LEN_WIDTH - 1 : 0]         free_mem_size,
    input  wire [CELL_ID_WIDTH - 1 : 0]     free_cell_id

);

// multi port memory free
wire                             free_port_debug_ready;

wire                             free_port_fifo_tready;
reg                              free_port_fifo_tready_reg;
wire                             free_port_fifo_req;
wire [LEN_WIDTH - 1 : 0]         free_port_fifo_size;
wire [CELL_ID_WIDTH -1 : 0]     free_port_fifo_cell_id;

assign free_port_fifo_tready = free_port_fifo_tready_reg;

axis_fifo #(
    .DEPTH(4),
    .DATA_WIDTH(LEN_WIDTH + CELL_ID_WIDTH),
    .KEEP_ENABLE(0),
    .KEEP_WIDTH(1),
    .LAST_ENABLE(0),
    .USER_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .FRAME_FIFO(0)
)
free_port0_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata({free_mem_size, free_cell_id}),
    .s_axis_tvalid(free_mem_req),
    .s_axis_tready(free_mem_ready),

    // AXI output
    .m_axis_tdata({free_port_fifo_size,free_port_fifo_cell_id}),
    .m_axis_tvalid(free_port_fifo_req),
    .m_axis_tready(free_port_fifo_tready)
);

reg [31:0] valid_cell_counter = 0;

reg [CELL_ID_WIDTH-1:0]           s_cell_fifo_tdata;
reg                                s_cell_fifo_tvalid;
wire                               s_cell_fifo_tready;

wire [CELL_ID_WIDTH-1:0]           m_cell_fifo_tdata;
wire                                m_cell_fifo_tvalid;
wire                                 m_cell_fifo_tready;

wire if_inc;
wire if_dec;
assign if_inc  = s_cell_fifo_tvalid && s_cell_fifo_tready;
assign if_dec  = m_cell_fifo_tvalid && m_cell_fifo_tready;

always@(posedge clk) begin
    if(rst) begin
        valid_cell_counter <= 0;
    end
    else begin
        if(if_inc && if_dec) begin
            valid_cell_counter <= valid_cell_counter;
        end
        else if(if_inc) begin
            valid_cell_counter <= valid_cell_counter +1;
        end
        else if (if_dec) begin
            valid_cell_counter <= valid_cell_counter -1;
        end
    end
end

axis_fifo #(
    .DEPTH(CELL_NUM + 10),
    .DATA_WIDTH(CELL_ID_WIDTH),
    .KEEP_ENABLE(0),
    .KEEP_WIDTH(1),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0)
)
cell_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(s_cell_fifo_tdata),
    .s_axis_tvalid(s_cell_fifo_tvalid),
    .s_axis_tready(s_cell_fifo_tready),

    // AXI output
    .m_axis_tdata(m_cell_fifo_tdata),
    .m_axis_tvalid(m_cell_fifo_tvalid),
    .m_axis_tready(m_cell_fifo_tready)
);

reg start;
reg [$clog2(CELL_NUM + 10) + 1 : 0] ini_count;
// reg [AXI_ADDR_WIDTH-1 : 0] ini_addr;

assign  alloc_cell_id = m_cell_fifo_tdata;
assign  alloc_mem_success = m_cell_fifo_tvalid && alloc_mem_req ;
assign  alloc_mem_intense = (valid_cell_counter <= 3);

assign m_cell_fifo_tready = alloc_mem_req;

always @(posedge clk) begin
    if(rst) begin // add free memory cell 
        start <= 1;
        ini_count <= 0;
        // ini_addr <= 0;
    end
    else begin
        s_cell_fifo_tvalid <= 0;
        // m_cell_fifo_tready <= 0;

        // initial add data block into the cell fifo
        if(start == 1 && ini_count < CELL_NUM ) begin
            if(s_cell_fifo_tready) begin
                s_cell_fifo_tvalid <= 1;
                ini_count <= ini_count + 1;
                s_cell_fifo_tdata <= ini_count;
                // ini_addr <= ini_addr + CELL_SIZE;
            end          
        end

        if(free_port_fifo_tready && free_port_fifo_req) begin
            s_cell_fifo_tvalid <= 1;
            s_cell_fifo_tdata <= free_port_fifo_cell_id;
        end
    end
end


always @* begin
    free_port_fifo_tready_reg = 0;

    if(s_cell_fifo_tready && free_port_fifo_req)
        free_port_fifo_tready_reg = 1;
end

endmodule