`timescale 1ns / 1ps
`include "define.v"

module reduce_tree #
(
    // given the reduce tree limitation, we assume the user queue size would not exceed 256
    parameter REDUCE_TREE_PTR_WIDTH = 8,
    parameter REDUCE_TREE_INDEX_WIDTH = 4,
    parameter  REDUCE_TREE_MASK_WIDTH = 2**REDUCE_TREE_INDEX_WIDTH,
    parameter RANK_BOUND = 2,
    parameter APP_COUNT =  (2**`APP_ID_WIDTH)
)
(
    input  wire                             clk,
    input  wire                             rst,

    /* queue dec update from queue manager*/
    input wire [`APP_ID_WIDTH-1 : 0]            s_axis_dec_app_id,
    input wire [REDUCE_TREE_INDEX_WIDTH-1:0]    s_axis_dec_core_id,
    input wire                                  s_axis_dec_valid,
    input wire [REDUCE_TREE_PTR_WIDTH-1:0]      s_axis_dec_length,


    input wire [REDUCE_TREE_INDEX_WIDTH-1:0]    s_axis_reset_core_id,
    input wire                                  s_axis_reset_valid,
    input wire                                  s_axis_reset_if_active,


    /* config app-to-core mask*/
    input wire [REDUCE_TREE_INDEX_WIDTH-1:0]    s_axis_set_app_core_id,
    input wire [`APP_ID_WIDTH-1:0]              s_axis_set_app_app_id,
    input wire [`PRIORITY_WIDTH-1:0]             s_axis_set_app_prio,
    input wire [2:0]                            s_axis_set_app_base_factor,
    input wire [2:0]                            s_axis_set_app_pree_factor,
    input wire                                  s_axis_set_app_core_valid,
    input wire                                  s_axis_set_app_core_if_active,

    input wire                                      s_findmin_req_en,
    input wire                                      if_policy_find_min,
    input wire [`APP_ID_WIDTH-1 : 0]                s_findmin_req_app_id,
    input wire [REDUCE_TREE_INDEX_WIDTH-1 : 0]      s_findmin_queue_id,
    output reg [REDUCE_TREE_INDEX_WIDTH-1 : 0]      m_findmin_result_core_id,
    output reg [`PRIORITY_WIDTH-1 : 0]              m_findmin_result_prio_id,
    output wire                                     m_findmin_if_drop,
    output reg [REDUCE_TREE_PTR_WIDTH-1 : 0]        m_findmin_result_ptr,
    output reg                                      m_findmin_result_en,
    output reg [`APP_ID_WIDTH-1 : 0]                m_findmin_result_app_id,
    

    output wire [APP_COUNT-1:0]                     m_app_mask,
    input wire [REDUCE_TREE_PTR_WIDTH-1:0]      rank_upbound

    // input wire                                      s_stall
    
);


parameter REDUCE_TREE_QUEUE_COUNT = 2**REDUCE_TREE_INDEX_WIDTH;
parameter PRIOTIY_COUNT  = 2**`PRIORITY_WIDTH;
// app state ram width = queue_depth_width * core_count
localparam PRIO_RAM_WDITH = REDUCE_TREE_PTR_WIDTH * REDUCE_TREE_QUEUE_COUNT;
reg [PRIO_RAM_WDITH-1:0] prio_queue_length_ram [PRIOTIY_COUNT-1 : 0];
reg [REDUCE_TREE_QUEUE_COUNT-1:0] prio_core_eligbility_mask [PRIOTIY_COUNT-1:0];
reg [APP_COUNT-1:0] app_eligbility_mask;
reg [`PRIORITY_WIDTH-1 : 0]  app_id_to_priority [APP_COUNT-1:0];
reg [REDUCE_TREE_QUEUE_COUNT-1 : 0]  app_core_mask [APP_COUNT-1:0];
reg [2 : 0]  app_id_to_pree_factor [APP_COUNT-1:0];
reg [2 : 0]  app_id_to_base_factor [APP_COUNT-1:0];
reg [REDUCE_TREE_QUEUE_COUNT-1 : 0]  if_drop;

// localparam APP_RAM_WDITH = REDUCE_TREE_PTR_WIDTH * REDUCE_TREE_QUEUE_COUNT;
// reg [APP_RAM_WDITH-1:0] app_queue_length_ram [APP_COUNT-1 : 0];
// reg [REDUCE_TREE_QUEUE_COUNT-1:0] app_core_eligbility_mask [APP_COUNT-1:0];

assign m_app_mask = app_eligbility_mask;

parameter REDUCE_TREE_LEVEL = REDUCE_TREE_INDEX_WIDTH;


reg [PRIO_RAM_WDITH-1:0]   queue_length_state;
reg [REDUCE_TREE_QUEUE_COUNT-1:0]   queue_mask_state;

reg [PRIO_RAM_WDITH-1:0]   queue_length_state_debug;


reg                         queue_req_delayed_min_en;
reg                         queue_req_delayed_rss_en;
reg [`PRIORITY_WIDTH-1 : 0] queue_req_delayed_prio_id;
reg [`APP_ID_WIDTH-1 : 0]   queue_req_delayed_app_id;
reg [REDUCE_TREE_INDEX_WIDTH-1 : 0]      queue_req_delayed_queue_id;

reg inc_valid;
reg [REDUCE_TREE_PTR_WIDTH-1:0]  inc_length;
reg [REDUCE_TREE_INDEX_WIDTH-1:0]  inc_core_id;
reg [`PRIORITY_WIDTH-1:0]  inc_prio_id;

reg dec_valid;
reg [REDUCE_TREE_PTR_WIDTH-1:0]  dec_length;
reg [REDUCE_TREE_INDEX_WIDTH-1:0]  dec_core_id;
reg [`PRIORITY_WIDTH-1:0]  dec_prio_id;
reg [2:0]  dec_base_factor;
reg [2:0]  dec_pree_factor;

reg [2:0]  inc_base_factor;
reg [2:0]  inc_pree_factor;


reg                                msg_rst_valid;  
reg                                msg_rst_if_active;  
reg [REDUCE_TREE_INDEX_WIDTH-1:0]  msg_rst_core_id;
reg [`PRIORITY_WIDTH-1:0]  msg_rst_prio_id;


integer i, j;

initial begin
    if_drop = {REDUCE_TREE_QUEUE_COUNT{1'b0}};
    app_eligbility_mask = {APP_COUNT{1'b0}};
    for (j = 0; j < PRIOTIY_COUNT; j = j + 1) begin
        prio_core_eligbility_mask[j] = {REDUCE_TREE_QUEUE_COUNT{1'b0}};
        prio_queue_length_ram[j] = {PRIO_RAM_WDITH{1'b1}};
    end

    for (i = 0; i < APP_COUNT; i = i + 1) begin
        app_id_to_priority[i] = {`PRIORITY_WIDTH{1'b0}};
        app_id_to_pree_factor[i] = 0;
        app_id_to_base_factor[i] = 0;
        app_core_mask[i] = {REDUCE_TREE_QUEUE_COUNT{1'b0}};
    end
end

always @(posedge clk) begin
    if(rst) begin
        msg_rst_valid <= 0;
        msg_rst_if_active <= 0;
        msg_rst_core_id <= 0;

        dec_valid  <= 0;
        dec_length <= 0; 
        dec_core_id     <= 0; 
        dec_prio_id     <= 0; 
        dec_base_factor <= 0;
        dec_pree_factor <= 0;


    end

    else begin
        dec_valid <= s_axis_dec_valid;
        dec_length <= s_axis_dec_length;
        dec_core_id <= s_axis_dec_core_id;
        dec_prio_id <= app_id_to_priority[s_axis_dec_app_id];

        dec_base_factor <= app_id_to_base_factor[s_axis_dec_app_id];
        dec_pree_factor <= app_id_to_pree_factor[s_axis_dec_app_id];

        msg_rst_valid <= s_axis_reset_valid;
        msg_rst_if_active <= s_axis_reset_if_active;
        msg_rst_core_id <= s_axis_reset_core_id;

    end
end

genvar  coreid, prioid;
generate
    for (coreid=0; coreid<REDUCE_TREE_QUEUE_COUNT; coreid = coreid + 1) begin: coreupdate
        reg [REDUCE_TREE_PTR_WIDTH - 1 : 0 ]tmp_value;

        for(prioid = 0; prioid < PRIOTIY_COUNT; prioid = prioid + 1) begin: prioupdate
            always @(posedge clk) begin
                if(msg_rst_valid && msg_rst_core_id == coreid) begin
                    if(msg_rst_if_active) begin
                        prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] <= {REDUCE_TREE_PTR_WIDTH{1'b0}};
                        prio_core_eligbility_mask[prioid][coreid] <= 1;
                    end
                    else begin
                        prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] <= {REDUCE_TREE_PTR_WIDTH{1'b1}};
                        prio_core_eligbility_mask[prioid][coreid] <= 0;
                    end
                end
                else if((dec_core_id == coreid && inc_core_id == coreid) &&  inc_valid && dec_valid)begin
                    tmp_value = prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH];
                    if(dec_prio_id >= prioid)begin
                        tmp_value = tmp_value - dec_length * dec_base_factor;
                    end
                    else begin
                        tmp_value = tmp_value - dec_length * dec_pree_factor;
                    end

                    if (inc_prio_id >= prioid) begin
                        tmp_value = tmp_value + inc_length * inc_base_factor;
                    end
                    else begin
                        tmp_value = tmp_value + inc_length * inc_pree_factor;
                    end
                    // tmp_value = prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] + inc_length - dec_length;
                    prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] <= tmp_value;
                    prio_core_eligbility_mask[prioid][coreid] <= (tmp_value < rank_upbound);
                end
                else begin
                    if((inc_core_id == coreid) && inc_valid) begin
                        if (inc_prio_id >= prioid) begin
                            tmp_value = inc_length * inc_base_factor;
                        end
                        else begin
                            tmp_value = inc_length * inc_pree_factor;
                        end

                        tmp_value = prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] + tmp_value;
                        prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] <= tmp_value;
                        prio_core_eligbility_mask[prioid][coreid] <= (tmp_value < rank_upbound);
                    end
                    if((dec_core_id == coreid) && dec_valid) begin
                        if(dec_prio_id >= prioid)begin
                            tmp_value = dec_length * dec_base_factor;
                        end
                        else begin
                            tmp_value = dec_length * dec_pree_factor;
                        end
                        tmp_value = prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] - tmp_value;
                        prio_queue_length_ram[prioid][coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] <= tmp_value;
                        prio_core_eligbility_mask[prioid][coreid] <= (tmp_value < rank_upbound);
                    end
                end
            end
            
        end

    end
endgenerate


genvar  setappid;
generate
    for (setappid=0; setappid<APP_COUNT; setappid = setappid + 1) begin: setappmask

        wire [REDUCE_TREE_QUEUE_COUNT-1:0] tmp_mask;
        assign tmp_mask = {REDUCE_TREE_QUEUE_COUNT{1'b0}} + 1'b1;
        always @(posedge clk) begin
            if(s_axis_set_app_core_valid && s_axis_set_app_core_if_active) begin
                    if(setappid == s_axis_set_app_app_id) begin
                        app_core_mask[setappid] <= app_core_mask[setappid] | (tmp_mask<< s_axis_set_app_core_id);
                        app_id_to_priority[setappid] <= s_axis_set_app_prio;
                        app_id_to_pree_factor[setappid] <= s_axis_set_app_pree_factor;
                        app_id_to_base_factor[setappid] <= s_axis_set_app_base_factor;
                    end
            end
            else if(s_axis_set_app_core_valid && !s_axis_set_app_core_if_active) begin
                    if(setappid == s_axis_set_app_app_id) begin
                        app_core_mask[setappid] <= app_core_mask[setappid] & (~(tmp_mask<< s_axis_set_app_core_id));
                    end
            end
        end
    end
endgenerate

genvar  appid;
generate
    for (appid=0; appid<APP_COUNT; appid = appid + 1) begin: appmaskreduce
       reg [`PRIORITY_WIDTH-1 : 0] app_prio;
       reg [REDUCE_TREE_QUEUE_COUNT-1:0] app_mask;
       always @(posedge clk) begin
           app_prio <= app_id_to_priority[appid];
           app_mask <= app_core_mask[appid];
           app_eligbility_mask[appid] <= |(prio_core_eligbility_mask[app_prio] & app_mask);
       end
    end
endgenerate


wire debug1;
wire debug2;
wire debug3;
wire debug4;
wire debug5;

assign debug1 = s_findmin_req_en;
assign debug2 = outer[4].req_en;
assign debug3 = outer[3].req_en;
assign debug4 = outer[2].req_en;
assign debug5 = outer[1].req_en;

reg [`PRIORITY_WIDTH-1 : 0] findmin_req_prio_id;

always @(posedge clk) begin
    
    if(rst) begin
        queue_length_state <= 0;
        queue_mask_state <= 0;
        queue_req_delayed_min_en <= 0;
        queue_req_delayed_rss_en <= 0;
        queue_req_delayed_prio_id <= 0;
        queue_req_delayed_app_id <= 0;
        queue_req_delayed_queue_id <= 0;
        findmin_req_prio_id  = 0;
    end
    else begin
        queue_length_state <= 0;
        queue_mask_state <= 0;
        queue_req_delayed_min_en <= 0;
        queue_req_delayed_rss_en <= 0;
        queue_req_delayed_prio_id <= 0;
        queue_req_delayed_app_id <= 0;
        queue_req_delayed_queue_id <= 0;
        findmin_req_prio_id = app_id_to_priority[s_findmin_req_app_id];
        if(s_findmin_req_en) begin
            queue_length_state <= prio_queue_length_ram[findmin_req_prio_id];
            queue_mask_state <= app_core_mask[s_findmin_req_app_id];
            queue_req_delayed_min_en <= s_findmin_req_en && if_policy_find_min;
            queue_req_delayed_rss_en <= s_findmin_req_en && !if_policy_find_min;
            queue_req_delayed_prio_id <= findmin_req_prio_id;
            queue_req_delayed_app_id <= s_findmin_req_app_id;
            queue_req_delayed_queue_id <= s_findmin_queue_id;
        end
    end

    queue_length_state_debug = prio_queue_length_ram[1];
end


// reg   random_reg [(2**(REDUCE_TREE_INDEX_WIDTH-1))-1:0];

localparam  LFSR_WIDTH = 64;
localparam  LFSR_DATA_WIDTH= 2**(REDUCE_TREE_INDEX_WIDTH-1);

reg [LFSR_WIDTH-1:0] state_reg = {LFSR_WIDTH{1'b1}};
reg [LFSR_DATA_WIDTH-1:0] random_reg ;

wire [LFSR_DATA_WIDTH-1:0] lfsr_data;
wire [LFSR_WIDTH-1:0] lfsr_state;

reg [LFSR_DATA_WIDTH - 1:0] cycle_counter;
always @(posedge clk) begin
    if (rst) begin
        state_reg <= {LFSR_WIDTH{1'b1}};
        cycle_counter <= 0;
    end 
    else begin
            cycle_counter <= cycle_counter + 1;
            state_reg <= lfsr_state;
            random_reg <= lfsr_state;
    end
end

lfsr #(
    .LFSR_WIDTH(LFSR_WIDTH),
    .LFSR_POLY(31'h10000001),
    .LFSR_CONFIG("FIBONACCI"),
    .LFSR_FEED_FORWARD(0),
    .REVERSE(0),
    .DATA_WIDTH(LFSR_DATA_WIDTH),
    .STYLE("AUTO")
)
lfsr_inst (
    .data_in(cycle_counter),
    .state_in(state_reg),
    .data_out(lfsr_data),
    .state_out(lfsr_state)
);


// assign m_findmin_req_ready = ! (s_stall && outer[4].req_en);
genvar level, tuple_offset, k;
generate
for (level=REDUCE_TREE_INDEX_WIDTH; level>=1; level=level-1) begin: outer
    reg [REDUCE_TREE_PTR_WIDTH-1:0] tmp_queue_length_state[(2**(level-1))-1:0];
    reg [REDUCE_TREE_INDEX_WIDTH-1:0] selected_queue [(2**(level-1))-1:0];
    reg req_en;
    reg [`PRIORITY_WIDTH-1:0] prio_id;
    reg [`APP_ID_WIDTH-1 : 0] app_id;

    if(level > 3) begin
        if(level < REDUCE_TREE_INDEX_WIDTH) begin
            always @(posedge clk) begin
                if(rst) begin
                    outer[level].req_en <= 0;
                    outer[level].prio_id <= 0;
                    outer[level].app_id <= 0;
                end
                else begin
                    outer[level].req_en <= outer[level+1].req_en;
                    outer[level].prio_id <= outer[level+1].prio_id;
                    outer[level].app_id <=  outer[level+1].app_id;
                end
            end
        end 
        else begin
            always @(posedge clk) begin
                if(rst) begin
                    outer[level].req_en <= 0;
                    outer[level].prio_id <= 0;
                    outer[level].app_id <= 0;
                end
                else begin
                    outer[level].req_en <= queue_req_delayed_min_en;
                    outer[level].prio_id <= queue_req_delayed_prio_id;
                    outer[level].app_id <= queue_req_delayed_app_id;
                end
            end
            
        end
    end
    else begin
        if(level < REDUCE_TREE_INDEX_WIDTH) begin
            always @(*) begin
                outer[level].req_en = outer[level+1].req_en;
                outer[level].prio_id = outer[level+1].prio_id;
                outer[level].app_id = outer[level+1].app_id;
            end
        end 
        else begin
            always @(*) begin
                outer[level].req_en = queue_req_delayed_min_en;
                outer[level].prio_id = queue_req_delayed_prio_id;
                outer[level].app_id = queue_req_delayed_app_id;
            end
            
        end
    end


    for (tuple_offset = 0; tuple_offset < 2**level; tuple_offset = tuple_offset + 2) begin: inner
        reg [REDUCE_TREE_PTR_WIDTH-1:0] cmp_left;
        reg [REDUCE_TREE_PTR_WIDTH-1:0] cmp_right;
        reg [REDUCE_TREE_INDEX_WIDTH-1:0] id_left;
        reg [REDUCE_TREE_INDEX_WIDTH-1:0] id_right;

        reg debug_inner;
        
        if(level > 3) begin
            if(level < REDUCE_TREE_INDEX_WIDTH) begin
                always @(*) begin
                    cmp_left = outer[level + 1].tmp_queue_length_state[tuple_offset];
                    cmp_right = outer[level + 1].tmp_queue_length_state[tuple_offset + 1];
                    id_left = outer[level + 1].selected_queue[tuple_offset];
                    id_right = outer[level + 1].selected_queue[tuple_offset + 1];
                end
            end
            else begin
                always @(*) begin
                    cmp_left = queue_mask_state[tuple_offset] ? queue_length_state[ tuple_offset * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] : {REDUCE_TREE_PTR_WIDTH{1'b1}};
                    cmp_right = queue_mask_state[tuple_offset + 1] ? queue_length_state[ (tuple_offset + 1) * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] : {REDUCE_TREE_PTR_WIDTH{1'b1}};
                    id_left = tuple_offset;
                    id_right = tuple_offset + 1;
                end
            end

            always @(posedge clk) begin
                if(cmp_left > cmp_right || ((cmp_left == cmp_right) && random_reg[tuple_offset/2] == 0)) begin
                    outer[level].tmp_queue_length_state[tuple_offset/2] <= cmp_right;
                    outer[level].selected_queue[tuple_offset/2] <= id_right;
                end
                else begin
                    outer[level].tmp_queue_length_state[tuple_offset/2] <= cmp_left;
                    outer[level].selected_queue[tuple_offset/2] <= id_left;
                end
            end
        end
        else begin
            always @(*) begin
                cmp_left = outer[level + 1].tmp_queue_length_state[tuple_offset];
                cmp_right = outer[level + 1].tmp_queue_length_state[tuple_offset + 1];
                id_left = outer[level + 1].selected_queue[tuple_offset];
                id_right = outer[level + 1].selected_queue[tuple_offset + 1];
            end

            always @(*) begin
                if(cmp_left > cmp_right || ((cmp_left == cmp_right) && random_reg[tuple_offset/2] == 0)) begin
                    outer[level].tmp_queue_length_state[tuple_offset/2] = cmp_right;
                    outer[level].selected_queue[tuple_offset/2] = id_right;
                end
                else begin
                    outer[level].tmp_queue_length_state[tuple_offset/2] = cmp_left;
                    outer[level].selected_queue[tuple_offset/2] = id_left;
                end
            end 
        end    
    end
end
endgenerate


genvar  rss_coreid;
generate
    for (rss_coreid=0; rss_coreid<REDUCE_TREE_QUEUE_COUNT; rss_coreid = rss_coreid + 1) begin: itercore

        always @(posedge clk) begin
            if((rss_coreid == queue_req_delayed_queue_id )&& queue_req_delayed_rss_en) begin
                if_drop[rss_coreid] <= (queue_length_state[rss_coreid * REDUCE_TREE_PTR_WIDTH +: REDUCE_TREE_PTR_WIDTH] >= rank_upbound);
            end
            else begin
                if_drop[rss_coreid] <= 0;
            end
        end

    end
endgenerate



always @(posedge clk) begin
    if(rst) begin
        inc_valid  <= 0;
        inc_length <= 0;
        inc_core_id  <= 0;
        inc_prio_id  <= 0;
        
        inc_base_factor <= 0;
        inc_pree_factor <= 0;
    end
    else begin
        if(if_policy_find_min) begin
            inc_valid  <= outer[1].req_en;
            inc_length <= 1;
            inc_core_id     <= outer[1].selected_queue[0];
            inc_prio_id     <= outer[1].prio_id;
            
            inc_base_factor <= app_id_to_base_factor[outer[1].app_id];
            inc_pree_factor <= app_id_to_pree_factor[outer[1].app_id];
        end
        else begin
            inc_valid  <= m_findmin_result_en && !m_findmin_if_drop;
            inc_length <= 1;
            inc_core_id     <= m_findmin_result_core_id;
            inc_prio_id     <= m_findmin_result_prio_id; 

            inc_base_factor <= app_id_to_base_factor[m_findmin_result_app_id];
            inc_pree_factor <= app_id_to_pree_factor[m_findmin_result_app_id];
        end

        if(if_policy_find_min) begin
            m_findmin_result_core_id <= outer[1].selected_queue[0];
            m_findmin_result_ptr <= outer[1].tmp_queue_length_state[0];
            m_findmin_result_en <= outer[1].req_en;
            m_findmin_result_prio_id <= outer[1].prio_id;
            m_findmin_result_app_id <= outer[1].app_id;
        end
        else begin
            m_findmin_result_core_id <= queue_req_delayed_queue_id;
            m_findmin_result_en <= queue_req_delayed_rss_en;
            m_findmin_result_prio_id <= queue_req_delayed_prio_id;
            m_findmin_result_app_id <= queue_req_delayed_app_id;
            m_findmin_result_ptr <= 0;
        end
    end

end

assign m_findmin_if_drop = (|if_drop) && !if_policy_find_min;

// ila_0 dispatch_cnt_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(s_axis_dec_valid), // input wire [0:0] probe0  
// 	.probe1({prio_core_eligbility_mask[0], prio_queue_length_ram[0][7:0]})
// );


endmodule
