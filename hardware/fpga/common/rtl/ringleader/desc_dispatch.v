`timescale 1ns / 1ps
`include "define.v"

module desc_dispatch #
(
    // Width of AXI data bus in bits
    parameter AXI_DATA_WIDTH = 512,
    parameter LEN_WIDTH = 16,
    parameter QUEUE_INDEX_WIDTH = 8,
    parameter QUEUE_PTR_WIDTH = 16,
    // we assume the user queue length would not exceed 256
    parameter REDUCE_TREE_PTR_WIDTH = 8,
    parameter REDUCE_TREE_INDEX_WIDTH = 4,
    parameter DMA_CLIENT_LEN_WIDTH = 20,
    parameter REQ_TAG_WIDTH = 8,
    // DMA RAM address width
    parameter RAM_ADDR_WIDTH = 16,
    parameter  REDUCE_TREE_MASK_WIDTH = 2**REDUCE_TREE_INDEX_WIDTH,
    parameter APP_COUNT =  (2**`APP_ID_WIDTH)
)
(
    input  wire                             clk,
    input  wire                             rst,

    /* input packet descriptor*/
    input  wire [`RL_DESC_WIDTH-1:0]             s_packet_desc,
    input  wire                                  s_packet_desc_valid,
    output reg                                   s_packet_desc_ready,


    /* output (scheduled) packet descriptor and queue id*/
    output  reg [QUEUE_INDEX_WIDTH-1:0]         m_axis_rx_req_queue,
    output  reg [`APP_ID_WIDTH-1:0]             m_axis_rx_req_app_id,
    output  reg [RAM_ADDR_WIDTH-1:0]            m_axis_rx_req_ram_addr,
    output  reg [DMA_CLIENT_LEN_WIDTH-1:0]      m_axis_rx_req_len,
    output  reg [15:0]                          m_axis_rx_csum,
    output  reg [31:0]                          m_axis_rx_hash,
    output  reg [REQ_TAG_WIDTH-1:0]             m_axis_rx_req_tag,
    output wire                                 m_packet_desc_valid,
    input  wire                                 m_packet_desc_ready,

    input wire [QUEUE_INDEX_WIDTH-1:0]       s_axis_cpu_msg_queue,
    input wire                               s_axis_cpu_msg_valid,
    input wire [`CPU_MSG_WIDTH-1:0]          s_axis_cpu_msg,

    input  wire [QUEUE_PTR_WIDTH-1:0]      rank_upbound,
    input  wire [3:0]                      sche_policy,

    input  wire [QUEUE_INDEX_WIDTH-1:0]    rss_mask_kernel_reg,
    input  wire [QUEUE_INDEX_WIDTH-1:0]    rss_mask_user_reg,

    input  wire [QUEUE_INDEX_WIDTH-1:0]    kernel_queue_offset_reg,
    input  wire [QUEUE_INDEX_WIDTH-1:0]    user_queue_offset_reg,

    input  wire                              free_mem_ready,
    output reg                               free_mem_req,
    output reg [LEN_WIDTH - 1 : 0]           free_mem_size,
    output reg [RAM_ADDR_WIDTH -1 : 0]       free_mem_addr,

    input wire                             nic_msg_valid,
    input wire [`NIC_MSG_WIDTH - 1 : 0]    nic_msg,
    output reg                             nic_msg_ready,

    output reg                            arm_cong_monitor,
    output reg                            arm_scale_down_monitor,
    output reg [`APP_ID_WIDTH-1:0]        arm_monitor_app_id,


    output wire [APP_COUNT-1:0]                     m_app_mask

    
);
parameter REDUCE_TREE_QUEUE_COUNT = 2**REDUCE_TREE_INDEX_WIDTH;
parameter REDUCE_TREE_LEVEL = REDUCE_TREE_INDEX_WIDTH;

reg [31:0] dispatch_income_count = 0;
reg [31:0] dispatch_outcome_count = 0;

always @(posedge clk) begin
    if(s_packet_desc_valid && s_packet_desc_ready) begin
        dispatch_income_count <= dispatch_income_count + 1;
    end

    if(m_packet_desc_valid && m_packet_desc_ready) begin
        dispatch_outcome_count <= dispatch_outcome_count + 1;
    end


end

initial begin
    if (QUEUE_INDEX_WIDTH != (REDUCE_TREE_INDEX_WIDTH)) begin
        $error("Error: Not aligend core_count; app_count and queue_count %h, %h, %h", QUEUE_INDEX_WIDTH, `APP_ID_WIDTH, REDUCE_TREE_INDEX_WIDTH);
        $finish;
    end
end

reg [REDUCE_TREE_INDEX_WIDTH-1 : 0]    reset_core_id;
reg [REDUCE_TREE_PTR_WIDTH-1 : 0]      reset_valid;
reg                                    reset_if_active;


reg [REDUCE_TREE_INDEX_WIDTH-1:0]    set_app_core_id;
reg [`APP_ID_WIDTH-1:0]              set_app_app_id;
reg [`PRIORITY_WIDTH-1:0]             set_app_prio;
reg [2:0]                             set_app_base_factor;
reg [2:0]                             set_app_pree_factor;
reg                                  set_app_core_valid;
reg                                  set_app_core_if_active;

reg [QUEUE_INDEX_WIDTH-1:0]     dec_core_id;
reg                             dec_valid;
reg [QUEUE_PTR_WIDTH-1:0]       dec_length;
reg [`APP_ID_WIDTH-1:0]         dec_app_id;

reg [`CPU_MSG_TYPE_SIZE-1 : 0] tmp_msg_type;

always @(*) begin
    tmp_msg_type = 0;
    if(s_axis_cpu_msg_valid) begin
        tmp_msg_type = s_axis_cpu_msg[`CPU_MSG_TYPE_OF +: `CPU_MSG_TYPE_SIZE];
    end
end

integer  i;
always @(posedge clk) begin
    reset_if_active <= 0;
    reset_valid <= 0;
    reset_core_id <= 0;

    set_app_core_id <= 0;
    set_app_app_id <= 0;
    set_app_prio <= 0;
    set_app_core_valid <= 0;
    set_app_core_if_active <= 0;

    set_app_base_factor <= 0;
    set_app_pree_factor <= 0;

    dec_app_id  <= 0;
    dec_core_id <= 0;
    dec_valid <= 0;
    dec_length <= 0;

    arm_cong_monitor <= 0;
    arm_scale_down_monitor <= 0;
    arm_monitor_app_id <= 0;

    if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_ENABLE_QUEUE)) begin
        reset_if_active <= 1;
        reset_valid <= 1;
        reset_core_id <= s_axis_cpu_msg_queue;
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_DISABLE_QUEUE)) begin
        reset_if_active <= 0;
        reset_valid <= 1;
        reset_core_id <= s_axis_cpu_msg_queue;
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_ACTIVATE_APP_THREAD))begin
        set_app_core_id <= s_axis_cpu_msg_queue;
        set_app_app_id <= s_axis_cpu_msg[`CPU_MSG_APP_ID_OF +: `CPU_MSG_APP_ID_SIZE];
        set_app_prio <= s_axis_cpu_msg[`CPU_MSG_APP_PRIO_OF +: `CPU_MSG_APP_PRIO_SIZE];
        set_app_base_factor <= s_axis_cpu_msg[`CPU_MSG_APP_BASE_FACTOR_OF +: `CPU_MSG_APP_BASE_FACTOR_SIZE];
        set_app_pree_factor <= s_axis_cpu_msg[`CPU_MSG_APP_PREE_FACTOR_OF +: `CPU_MSG_APP_PREE_FACTOR_SIZE];

        set_app_core_valid <= 1;
        set_app_core_if_active <= 1;
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_DEACTIVATE_APP_THREAD))begin
        set_app_core_id <= s_axis_cpu_msg_queue;
        set_app_app_id <= s_axis_cpu_msg[`CPU_MSG_APP_ID_OF +: `CPU_MSG_APP_ID_SIZE];
        set_app_core_valid <= 1;
        set_app_core_if_active <= 0;
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_FEEDBACK_THREAD))begin
        dec_app_id  <= s_axis_cpu_msg[`CPU_MSG_APP_ID_OF +: `CPU_MSG_APP_ID_SIZE];
        dec_core_id <= s_axis_cpu_msg_queue;
        dec_valid <= 1;
        dec_length <= s_axis_cpu_msg[`CPU_MSG_APP_CONTENT_OF +: `CPU_MSG_APP_CONTENT_SIZE];
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_ARM_CONG_MONITOR)) begin
        arm_cong_monitor <= 1;
        arm_monitor_app_id <= s_axis_cpu_msg[`CPU_MSG_APP_ID_OF +: `CPU_MSG_APP_ID_SIZE];
    end
    else if(s_axis_cpu_msg_valid && (tmp_msg_type == `CPU_MSG_ARM_SCALE_DOWN_MONITOR)) begin
        arm_scale_down_monitor <= 1;
        arm_monitor_app_id <= s_axis_cpu_msg[`CPU_MSG_APP_ID_OF +: `CPU_MSG_APP_ID_SIZE];
    end
end


wire                                    min_queue_req_en;
wire [`APP_ID_WIDTH-1 : 0]              min_queue_req_app_id;

wire [REDUCE_TREE_INDEX_WIDTH-1 : 0]    min_core_id;
wire [REDUCE_TREE_INDEX_WIDTH-1 : 0]    min_queue_req_queue_id;
wire [`PRIORITY_WIDTH-1 : 0]            min_prio_id;
wire                                    if_drop;
wire [REDUCE_TREE_PTR_WIDTH-1 : 0]      min_queue_length;
wire                                    min_queue_en;

reg                                     drop;


// Reduce tree to find the least loaded core.
reduce_tree#(
    .REDUCE_TREE_PTR_WIDTH(REDUCE_TREE_PTR_WIDTH),
    .REDUCE_TREE_INDEX_WIDTH(REDUCE_TREE_INDEX_WIDTH),
    .REDUCE_TREE_MASK_WIDTH(REDUCE_TREE_MASK_WIDTH)
)
reduce_tree_inst(
    .clk(clk),
    .rst(rst),

    .s_axis_dec_app_id(dec_app_id),
    .s_axis_dec_core_id(dec_core_id),
    .s_axis_dec_valid(dec_valid),
    .s_axis_dec_length(dec_length),

    .s_axis_reset_core_id(reset_core_id),
    .s_axis_reset_valid(reset_valid),
    .s_axis_reset_if_active(reset_if_active),

    .s_axis_set_app_core_id(set_app_core_id),
    .s_axis_set_app_app_id(set_app_app_id),
    .s_axis_set_app_prio(set_app_prio),
    .s_axis_set_app_base_factor(set_app_base_factor),
    .s_axis_set_app_pree_factor(set_app_pree_factor),
    .s_axis_set_app_core_valid(set_app_core_valid),
    .s_axis_set_app_core_if_active(set_app_core_if_active),


    .s_findmin_req_en(min_queue_req_en),
    .if_policy_find_min(sche_policy == 4'b1),
    .s_findmin_req_app_id(min_queue_req_app_id),
    .s_findmin_queue_id(min_queue_req_queue_id),
    .m_findmin_result_prio_id(min_prio_id),
    .m_findmin_result_core_id(min_core_id),
    .m_findmin_result_ptr(min_queue_length),
    .m_findmin_if_drop(if_drop),
    .m_findmin_result_en(min_queue_en),

    .m_app_mask(m_app_mask),
    .rank_upbound(rank_upbound)
);


wire                                 s_out_fifo_packet_desc_ready;

wire [`RL_DESC_WIDTH-1:0]            m_out_fifo_packet_desc;
wire                                 m_out_fifo_packet_desc_ready;
wire                                 m_out_fifo_packet_desc_valid;

wire [`RL_DESC_WIDTH-1:0]            s_min_fifo_packet_desc;
wire                                 s_min_fifo_packet_desc_ready;
wire                                 s_min_fifo_packet_desc_valid;

wire [`RL_DESC_WIDTH-1:0]            m_min_fifo_packet_desc;
wire                                 m_min_fifo_packet_desc_ready;
wire                                 m_min_fifo_packet_desc_valid;
wire [REDUCE_TREE_INDEX_WIDTH-1 : 0] m_min_fifo_min_core_id;

reg  [7:0]                           out_fifo_counter;
wire out_fifo_counter_dec;
wire out_fifo_counter_inc;
wire out_fifo_counter_drop_dec;

assign    out_fifo_counter_dec = m_min_fifo_packet_desc_valid && m_min_fifo_packet_desc_ready;
assign    out_fifo_counter_drop_dec = if_drop;
assign    out_fifo_counter_inc = s_packet_desc_valid && s_packet_desc_ready;
reg  [9:0]                           out_total_counter;
reg  [9:0]                           qm_dec_counter;

always @(*) begin
    s_packet_desc_ready = s_out_fifo_packet_desc_ready && out_fifo_counter < 4;
end

assign min_queue_req_en = s_packet_desc_valid && s_packet_desc_ready;
assign min_queue_req_app_id = s_packet_desc[`RL_DESC_APP_ID_OF +: `RL_DESC_APP_ID_SIZE];

assign min_queue_req_queue_id = s_packet_desc[`RL_DESC_HASH_OF   +: `RL_DESC_HASH_SIZE] & rss_mask_user_reg;

axis_fifo #(
    .DEPTH(8),
    .DATA_WIDTH(`RL_DESC_WIDTH),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0),
    .PIPELINE_OUTPUT(1)
)
in_find_min_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata(s_packet_desc),
    .s_axis_tvalid(min_queue_req_en),
    .s_axis_tready(s_out_fifo_packet_desc_ready),

    // AXI output
    .m_axis_tdata(m_out_fifo_packet_desc),
    .m_axis_tvalid(m_out_fifo_packet_desc_valid),
    .m_axis_tready(m_out_fifo_packet_desc_ready)
);

always @(posedge clk) begin
    if(rst) begin
        out_fifo_counter = 0;
        out_total_counter = 0;
        qm_dec_counter = 0;
    end
    else begin
        if(dec_valid) begin
           qm_dec_counter <= dec_length + qm_dec_counter;
        end

        if(out_fifo_counter_inc) begin
            out_total_counter <= out_total_counter + 1;
        end

        if(out_fifo_counter_dec && out_fifo_counter_inc && out_fifo_counter_drop_dec) begin
            out_fifo_counter <= out_fifo_counter -1;
        end
        else if(out_fifo_counter_inc && (out_fifo_counter_dec || out_fifo_counter_drop_dec)) begin
            out_fifo_counter <= out_fifo_counter;
        end
        else if(out_fifo_counter_inc) begin
            out_fifo_counter <= out_fifo_counter + 1;
        end
        else if(out_fifo_counter_dec && out_fifo_counter_drop_dec) begin
            out_fifo_counter <= out_fifo_counter - 2;
        end
        else if(out_fifo_counter_dec || out_fifo_counter_drop_dec) begin
            out_fifo_counter <= out_fifo_counter - 1;
        end
    end
end

// assign m_out_fifo_packet_desc_ready = m_packet_desc_ready && min_queue_en;
// assign m_packet_desc_valid = m_out_fifo_packet_desc_valid && m_out_fifo_packet_desc_ready && !if_drop;

assign m_out_fifo_packet_desc_ready = s_min_fifo_packet_desc_ready && min_queue_en;
assign s_min_fifo_packet_desc_valid = m_out_fifo_packet_desc_valid && m_out_fifo_packet_desc_ready && !if_drop;
assign s_min_fifo_packet_desc = m_out_fifo_packet_desc;

axis_fifo #(
    .DEPTH(16),
    .DATA_WIDTH(`RL_DESC_WIDTH + REDUCE_TREE_INDEX_WIDTH),
    .KEEP_ENABLE(0),
    .LAST_ENABLE(0),
    .ID_ENABLE(0),
    .DEST_ENABLE(0),
    .USER_ENABLE(0),
    .FRAME_FIFO(0),
    .PIPELINE_OUTPUT(1)
)
out_fifo (
    .clk(clk),
    .rst(rst),

    // AXI input
    .s_axis_tdata({s_min_fifo_packet_desc, min_core_id}),
    .s_axis_tvalid(s_min_fifo_packet_desc_valid),
    .s_axis_tready(s_min_fifo_packet_desc_ready),

    // AXI output
    .m_axis_tdata({m_min_fifo_packet_desc, m_min_fifo_min_core_id}),
    .m_axis_tvalid(m_min_fifo_packet_desc_valid),
    .m_axis_tready(m_min_fifo_packet_desc_ready)
);

assign m_min_fifo_packet_desc_ready = m_packet_desc_ready;
assign m_packet_desc_valid = m_min_fifo_packet_desc_valid;

always @(*) begin
    m_axis_rx_req_queue = 0;
    m_axis_rx_req_ram_addr = 0;
    m_axis_rx_req_len = 0;
    m_axis_rx_csum = 0;
    m_axis_rx_hash = 0;
    m_axis_rx_req_tag = 0;
    m_axis_rx_req_app_id = 0;
    free_mem_req = 0;
    free_mem_size = 0;
    free_mem_addr = 0;
    nic_msg_ready = m_min_fifo_packet_desc_valid && m_min_fifo_packet_desc_ready;


    if(m_min_fifo_packet_desc_valid && m_min_fifo_packet_desc_ready) begin

        m_axis_rx_req_ram_addr = m_min_fifo_packet_desc[`RL_DESC_CELL_ID_OF  +: `RL_DESC_CELL_ID_SIZE] * `RL_CELL_SIZE;
        m_axis_rx_req_len = m_min_fifo_packet_desc[`RL_DESC_LEN_OF   +: `RL_DESC_LEN_SIZE];
        m_axis_rx_csum = m_min_fifo_packet_desc[`RL_DESC_CSUM_OF   +: `RL_DESC_CSUM_SIZE];
        // m_axis_rx_hash = m_min_fifo_packet_desc[`RL_DESC_HASH_OF   +: `RL_DESC_HASH_SIZE];
        if(nic_msg_valid) begin
            m_axis_rx_hash = nic_msg;
        end

        m_axis_rx_req_app_id = m_min_fifo_packet_desc[`RL_DESC_APP_ID_OF +: `RL_DESC_APP_ID_SIZE];

        m_axis_rx_req_tag = 0;

        m_axis_rx_req_queue = m_min_fifo_min_core_id;

    end
  
    // drop before go into min fifo
    if(if_drop && min_queue_en) begin
        free_mem_req = 1;
        free_mem_size = 1;
        free_mem_addr = m_out_fifo_packet_desc[`RL_DESC_CELL_ID_OF  +: `RL_DESC_CELL_ID_SIZE] * `RL_CELL_SIZE;
    end
end


// ila_0 dispatch_perf_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(m_packet_desc_ready && m_packet_desc_valid), // input wire [0:0] probe0  
// 	.probe1({m_axis_rx_req_queue, m_packet_desc_valid, m_axis_rx_req_app_id, min_queue_req_app_id, min_queue_req_en, m_packet_desc_ready, m_app_mask, sche_policy, out_fifo_counter, s_packet_desc_ready, s_packet_desc_valid, m_out_fifo_packet_desc_valid, m_out_fifo_packet_desc_ready, s_min_fifo_packet_desc_valid, s_min_fifo_packet_desc_ready, m_min_fifo_packet_desc_valid, m_min_fifo_packet_desc_ready})
// );


endmodule
