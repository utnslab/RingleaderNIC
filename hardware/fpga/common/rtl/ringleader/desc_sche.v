`timescale 1ns / 1ps
`include "define.v"

module desc_sche #
(
    // Width of AXI data bus in bits
    parameter APP_ELI_MASK_WIDTH = 2** `APP_ID_WIDTH
)
(
    input  wire                             clk,
    input  wire                             rst,

    /* output (scheduled) packet descriptor*/
    output wire [`RL_DESC_WIDTH-1:0]             m_packet_desc,
    output wire                                  m_packet_desc_valid,
    input  wire                                  m_packet_desc_ready,

    /* request to queue manager*/
    /* output (scheduled) packet descriptor*/
    input wire [`RL_DESC_WIDTH-1:0]               qm_packet_desc,
    input wire                                    qm_packet_desc_valid,
    output wire                                   qm_packet_desc_req,
    output reg [`RL_DESC_APP_ID_SIZE-1:0]         qm_packet_desc_app_id,

    input wire                                  s_pifo_valid,
    input wire [`RL_DESC_APP_ID_SIZE-1:0]       s_pifo_prio, 
    input wire [`RL_DESC_APP_ID_SIZE-1:0]       s_pifo_data,
    output wire                                 s_pifo_ready,

    input wire [APP_ELI_MASK_WIDTH-1:0]         s_app_mask
);

wire                                  fake_pifo_valid;
wire [`RL_DESC_APP_ID_SIZE-1:0]       fake_pifo_prio;
wire [`RL_DESC_APP_ID_SIZE-1:0]       fake_pifo_data;
wire                                 fake_pifo_ready;

assign s_pifo_ready = 1;

axis_fifo #(
    .DEPTH(16),
    .DATA_WIDTH(`RL_DESC_APP_ID_SIZE),
    .KEEP_ENABLE(0),
    .KEEP_WIDTH(1),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0)
)
fake_pifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(s_pifo_data),
    .s_axis_tvalid(s_pifo_valid),
    .s_axis_tready(),

    // AXI output
    .m_axis_tdata(fake_pifo_data),
    .m_axis_tvalid(fake_pifo_valid),
    .m_axis_tready(fake_pifo_ready)
);

assign fake_pifo_ready = s_app_mask[0] && m_packet_desc_ready;

assign qm_packet_desc_req = fake_pifo_valid && fake_pifo_ready;

assign m_packet_desc = qm_packet_desc;
assign m_packet_desc_valid = qm_packet_desc_valid;

always @(*) begin
    qm_packet_desc_app_id = 0;
    if(fake_pifo_valid) begin
        qm_packet_desc_app_id = fake_pifo_data;
    end
end


endmodule