`timescale 1ns / 1ps
`include "define.v"

module desc_sche_pifo #
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
    input wire                                  s_pifo_empty,
    input wire [`RL_DESC_APP_ID_SIZE-1:0]       s_pifo_empty_data,

    input wire [APP_ELI_MASK_WIDTH-1:0]         s_app_mask
);

wire                                  pifo_valid;
wire [`RL_DESC_APP_ID_SIZE-1:0]       pifo_prio;
wire [`RL_DESC_APP_ID_SIZE-1:0]       pifo_data;
wire                                  pifo_ready;



assign pifo_ready = m_packet_desc_ready;
assign qm_packet_desc_req = pifo_valid && pifo_ready;

assign m_packet_desc = qm_packet_desc;
assign m_packet_desc_valid = qm_packet_desc_valid;

always @(*) begin
    qm_packet_desc_app_id = 0;
    if(pifo_valid) begin
        qm_packet_desc_app_id = pifo_data;
    end
end

pifo_warp #(
    .NUMPIFO    (16),
    .BITPORT    (1),
    .BITPRIO    (`RL_DESC_APP_ID_SIZE),
    .BITDESC    (`RL_DESC_APP_ID_SIZE),
    .BITMASK    (APP_ELI_MASK_WIDTH),
    .PIFO_ID    (0),
    .SMALL_PK_OPT (0)
) pf_inst (
    .clk                                (clk),
    .rst                                (rst),

    .pifo_in_ready                      (s_pifo_ready),
    .pifo_in_valid                      (s_pifo_valid),
    .pifo_in_prio                       (s_pifo_prio), 
    .pifo_in_data                       (s_pifo_data), 
    .pifo_in_drop                       (0),
    .pifo_in_empty                      (s_pifo_empty),
    .pifo_in_empty_data                 (s_pifo_empty_data),


    .pifo_out_ready                      (pifo_ready),
    .pifo_out_valid                      (pifo_valid),
    .pifo_out_prio                       (), 
    .pifo_out_data                       (pifo_data),

    .pifo_out_drop_valid                 (),
    .pifo_out_drop_prio                  (), 
    .pifo_out_drop_data                  (),

    .entry_mask                          (s_app_mask)
);



// pifo #(
//     .NUMPIFO    (1024),
//     .BITPORT    (1),
//     .BITPRIO    (`RL_DESC_PRIO_SIZE),
//     .BITDATA    (`RL_DESC_WIDTH),
//     .PIFO_ID    (0)
// )inst(
//     .clk(clk),
//     .rst(rst),
//     .pop_0(m_packet_desc_ready), 
//     .oprt_0(0), 
//     .ovld_0(m_packet_desc_valid), 
//     .opri_0(), 
//     .odout_0(m_packet_desc),
    
//     .push_1(s_packet_desc_valid), 
//     .uprt_1(0), 
//     .upri_1(0), 
//     .udin_1(s_packet_desc),
//     .push_1_drop(0),
    
        
//     .push_2(0),
//     .push_2_drop(0),

//     .odrop_vld_0(),
//     .odrop_pri_0(),
//     .odrop_dout_0()
// );
// assign s_packet_desc_ready = 1;


endmodule