`timescale 1ns / 1ps
`include "define.v"

module pifo_queue_manager #
(
    // Width of AXI data bus in bits
    parameter QUEUE_NUMBER_WIDTH = 6,
    parameter PER_QUEUE_RAM_SIZE = 64,
    parameter LEN_WIDTH = 16,
    parameter RAM_ADDR_WIDTH = 16,
    parameter CELL_NUM = 64
)
(
    input  wire                             clk,
    input  wire                             rst,

    /* input packet descriptor*/
    input  wire [`RL_DESC_WIDTH-1:0]             s_packet_desc,
    input  wire                                  s_packet_desc_valid,
    input  wire [`RL_DESC_APP_ID_SIZE-1:0]       s_packet_desc_app_id,
    output wire                                  s_packet_desc_ready,

    /* output (scheduled) packet descriptor*/
    input  wire                                  m_packet_desc_req,
    input  wire [`RL_DESC_APP_ID_SIZE-1:0]       m_packet_desc_app_id,
    output wire [`RL_DESC_WIDTH-1:0]             m_packet_desc,
    output wire                                  m_packet_desc_valid,


    /* output queue handler to PFIO*/
    output wire                                  m_pifo_valid,
    output wire [`RL_DESC_APP_ID_SIZE-1:0]       m_pifo_prio, 
    output wire [`RL_DESC_APP_ID_SIZE-1:0]       m_pifo_data,
    input wire                                   m_pifo_ready,
    output wire                                  m_pifo_empty,
    output wire [`RL_DESC_APP_ID_SIZE-1:0]       m_pifo_empty_data,

    input  wire                              free_mem_ready,
    output wire                               free_mem_req,
    output wire [LEN_WIDTH - 1 : 0]           free_mem_size,
    output wire [RAM_ADDR_WIDTH -1 : 0]       free_mem_addr,

   input wire                              reset_monitor,
   input wire                              config_monitor,
   input wire [`APP_MSG_APP_ID_SIZE-1:0]     config_monitor_app_id,
   input wire [4:0]                        config_scale_down_epoch_log,
   input wire [4:0]                        config_cong_dectect_epoch_log,
   input wire [3:0]                        config_scale_down_thresh,

    input wire                            arm_cong_monitor,
    input wire                            arm_scale_down_monitor,
    input wire [`APP_ID_WIDTH-1:0]        arm_monitor_app_id,

    output wire                             nic_msg_valid,
    output wire [`NIC_MSG_WIDTH - 1 : 0]    nic_msg,
    input wire                              nic_msg_ready

);

reg [9:0] queue_size [0:  QUEUE_NUMBER - 1]; 
wire [7:0] queue_debug [0:  QUEUE_NUMBER - 1]; 

localparam QUEUE_NUMBER = 2 ** QUEUE_NUMBER_WIDTH;
localparam RAM_ENTY_NUM = QUEUE_NUMBER *  PER_QUEUE_RAM_SIZE;
localparam QUEUE_PTR_WIDTH = $clog2(PER_QUEUE_RAM_SIZE);

reg [2:0] debug;
reg  [(3 + `RL_DESC_WIDTH - 1):0] array_ctrl [0 : QUEUE_NUMBER - 1];
reg  [(`RL_DESC_PRIO_SIZE- 1):0] prio_ctrl [0 : QUEUE_NUMBER - 1];


integer i;
initial begin
  for (i=0; i < QUEUE_NUMBER; i=i+1)
    queue_size[i] = 0;
  queue_size[0] = CELL_NUM/16;
  queue_size[1] = (CELL_NUM * 7) /16;
  queue_size[2] = (CELL_NUM * 7) /16;
  queue_size[3] = CELL_NUM/16;
end

reg [QUEUE_NUMBER - 1 : 0] pifo_enqueue_arb_valid;
wire [QUEUE_NUMBER - 1 : 0] pifo_enqueue_arb_grant;
wire [QUEUE_NUMBER_WIDTH -1 : 0] arbiter_grant_encode ;
wire arbiter_grant_valid;

arbiter  #(
    .PORTS(QUEUE_NUMBER),
    .ARB_BLOCK(0),
    .ARB_BLOCK_ACK(0)
)
handler_arbiter (
    .clk(clk),
    .rst(rst),
    // AXI inputs
    .request(pifo_enqueue_arb_valid),
    .acknowledge(),
    .grant(pifo_enqueue_arb_grant),
    .grant_valid(arbiter_grant_valid),
    .grant_encoded(arbiter_grant_encode)
);

assign m_pifo_valid = arbiter_grant_valid;
assign m_pifo_data = arbiter_grant_encode[`RL_DESC_APP_ID_SIZE-1:0];
// TODO: compute rank here --..
assign m_pifo_prio = prio_ctrl[arbiter_grant_encode[`RL_DESC_APP_ID_SIZE-1:0]];
// assume ready always == 1;

assign m_pifo_empty = array_ctrl[m_packet_desc_app_id][`RL_DESC_WIDTH + 2];
assign m_pifo_empty_data = m_packet_desc_app_id;
assign s_packet_desc_ready = 1;
assign m_packet_desc_valid = array_ctrl[m_packet_desc_app_id][`RL_DESC_WIDTH];
assign m_packet_desc = array_ctrl[m_packet_desc_app_id][`RL_DESC_WIDTH-1 : 0];

assign free_mem_req = array_ctrl[s_packet_desc_app_id][`RL_DESC_WIDTH + 1];
assign free_mem_size = 1;
assign free_mem_addr = s_packet_desc[`RL_DESC_CELL_ID_OF  +: `RL_DESC_CELL_ID_SIZE] * `RL_CELL_SIZE;



reg [QUEUE_NUMBER - 1 : 0] nic_msg_arb_valid;
wire [QUEUE_NUMBER - 1 : 0] nic_msg_arb_grant;
wire [QUEUE_NUMBER_WIDTH -1 : 0] nic_msg_grant_encode;
wire nic_msg_grant_valid;

reg  [(`NIC_MSG_WIDTH- 1):0] nic_msg_ctrl [0 : QUEUE_NUMBER - 1];


arbiter  #(
    .PORTS(QUEUE_NUMBER),
    .ARB_BLOCK(0),
    .ARB_BLOCK_ACK(0)
)
nic_msg_arbiter (
    .clk(clk),
    .rst(rst),
    // AXI inputs
    .request(nic_msg_arb_valid),
    .acknowledge(),
    .grant(nic_msg_arb_grant),
    .grant_valid(nic_msg_grant_valid),
    .grant_encoded(nic_msg_grant_encode)
);


assign nic_msg_valid = nic_msg_grant_valid;
assign nic_msg = nic_msg_ctrl[nic_msg_grant_encode[`RL_DESC_APP_ID_SIZE-1:0]];

genvar queue_n;
generate
for (queue_n=0; queue_n<QUEUE_NUMBER; queue_n=queue_n + 1) begin: array_
    reg queue_priority[`RL_DESC_PRIO_SIZE-1 : 0];
    reg [9:0] fifo_counter;
    reg in_app_fifo_valid, inc_cnt, dec_cnt, if_free;
    wire in_app_fifo_ready;
    reg [`RL_DESC_WIDTH-1:0] in_app_fifo_data;

    wire out_app_fifo_valid;
    reg out_app_fifo_ready;
    wire [`RL_DESC_WIDTH-1:0] out_app_fifo_data;

    wire                             msg_en;
    wire [`NIC_MSG_WIDTH - 1 : 0]        msg;
    
    wire                             o_msg_en;
    wire [`NIC_MSG_WIDTH - 1 : 0]    o_msg;

    reg if_handler_oustand;
    
    always @(*) begin
        pifo_enqueue_arb_valid[queue_n] = 0;
        if(out_app_fifo_valid && !if_handler_oustand && m_pifo_ready && !pifo_enqueue_arb_grant[queue_n]) begin
                pifo_enqueue_arb_valid[queue_n] = 1;
                prio_ctrl[queue_n] = out_app_fifo_data[`RL_DESC_PRIO_OF  +: `RL_DESC_PRIO_SIZE];
        end
    end

    always @(posedge clk) begin
        if(rst) begin
            if_handler_oustand <= 0;
        end
        else begin
            if(pifo_enqueue_arb_grant[queue_n]) begin
                if_handler_oustand <= 1;
            end
            if(out_app_fifo_ready) begin
                if_handler_oustand <= 0;
            end
        end
    end
    
    always @(*) begin
        in_app_fifo_data  = s_packet_desc;
        if_free = fifo_counter >= queue_size[queue_n] - 1;
        in_app_fifo_valid = s_packet_desc_valid ? (queue_n == s_packet_desc_app_id) : 0;
        out_app_fifo_ready = m_packet_desc_req ?  (queue_n == m_packet_desc_app_id) : 0;

        array_ctrl[queue_n] = {!out_app_fifo_valid && out_app_fifo_ready , in_app_fifo_valid && if_free, out_app_fifo_valid && out_app_fifo_ready, out_app_fifo_data};

        inc_cnt = in_app_fifo_valid && in_app_fifo_ready && !if_free;
        dec_cnt = out_app_fifo_valid && out_app_fifo_ready;
    end

    always @(posedge clk) begin
        if(rst) begin
            fifo_counter <= 0;
        end
        else begin
            if(inc_cnt && dec_cnt)begin
                fifo_counter <= fifo_counter;
            end
            else if(inc_cnt) begin
                fifo_counter <= fifo_counter + 1;
            end
            else if(dec_cnt) begin
                fifo_counter <= fifo_counter - 1;
            end
        end
    end

    assign queue_debug[queue_n] = fifo_counter;

    axis_fifo #(
        .DEPTH(PER_QUEUE_RAM_SIZE),
        .DATA_WIDTH(`RL_DESC_WIDTH),
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
        .s_axis_tdata(in_app_fifo_data),
        .s_axis_tvalid(in_app_fifo_valid && !if_free),
        .s_axis_tready(in_app_fifo_ready),

        // AXI output
        .m_axis_tdata(out_app_fifo_data),
        .m_axis_tvalid(out_app_fifo_valid),
        .m_axis_tready(out_app_fifo_ready)
    );


    perf_monitor #(
        .APP_ELI_MASK_WIDTH(0)
    )
    perf_monitor_ (
        .clk(clk),
        .rst(rst),


       .app_id(queue_n),
       .enqueue_packet(in_app_fifo_valid && in_app_fifo_ready && !if_free),
       .dequeue_packet(out_app_fifo_valid && out_app_fifo_ready),
       .reset_monitor(reset_monitor),
       .config_monitor(config_monitor && (config_monitor_app_id == queue_n)),
       .config_scale_down_epoch_log(config_scale_down_epoch_log),
       .config_cong_dectect_epoch_log(config_cong_dectect_epoch_log),
       .config_scale_down_thresh(config_scale_down_thresh),
       .arm_cong_monitor(arm_cong_monitor && (arm_monitor_app_id == queue_n)),
       .arm_scale_down_monitor(arm_scale_down_monitor && (arm_monitor_app_id == queue_n)),

       .msg_en(msg_en),
       .msg(msg)
    );


    axis_fifo #(
        .DEPTH(4),
        .DATA_WIDTH(`NIC_MSG_WIDTH),
        .KEEP_ENABLE(0),
        .LAST_ENABLE(0),
        .ID_ENABLE(0),
        .DEST_ENABLE(0),
        .USER_ENABLE(0),
        .FRAME_FIFO(0)
    )
    nic_msg_fifo (
        .clk(clk),
        .rst(rst),

        // AXI input
        .s_axis_tdata(msg),
        .s_axis_tvalid(msg_en),
        .s_axis_tready(),

        // AXI output
        .m_axis_tdata(o_msg),
        .m_axis_tvalid(o_msg_en),
        .m_axis_tready(nic_msg_arb_grant[queue_n])
    );

    always @(*) begin
        nic_msg_arb_valid[queue_n] = 0;
        if(o_msg_en && !nic_msg_arb_grant[queue_n] && nic_msg_ready) begin
                nic_msg_arb_valid[queue_n] = 1;
                nic_msg_ctrl[queue_n] = o_msg;
        end
    end

end
    
endgenerate

// ila_0 queue_perf_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(s_packet_desc_valid), // input wire [0:0] probe0  
// 	.probe1({s_packet_desc_valid, s_packet_desc_app_id, m_packet_desc_req, m_packet_desc_app_id, m_packet_desc_valid, m_pifo_valid, m_pifo_prio, m_pifo_ready, m_pifo_empty, m_pifo_empty_data, free_mem_ready, free_mem_req, queue_size[1], queue_debug[1], queue_debug[2]})
// );


endmodule