`timescale 1ns / 1ps
`include "define.v"

module header_parser #
(
    // Width of AXI stream interfaces in bits
    parameter DATA_WIDTH = 256,
    // AXI stream tkeep signal width (words per cycle)
    parameter KEEP_WIDTH = (DATA_WIDTH/8)
)
(
    input  wire                       clk,
    input  wire                       rst,

    input wire  [4*8-1:0]         user_space_ip,
    input wire                     app_config_msg_en,
    input wire [`APP_MSG_APP_ID_SIZE-1:0]    app_config_msg_app_id,
    input wire [`APP_MSG_APP_PORT_SIZE-1: 0] app_config_msg_port,
    input wire [`APP_MSG_APP_PRIO_SIZE-1:0]  app_config_msg_app_prio,
    /*
     * AXI input
     */
    input  wire [DATA_WIDTH-1:0]  s_axis_tdata,
    input  wire [KEEP_WIDTH-1:0]  s_axis_tkeep,
    input  wire                   s_axis_tvalid,
    input  wire                   s_axis_tlast,

    output reg [`RL_DESC_PRIO_SIZE -1 : 0]          m_desc_prio,
    output reg [`RL_DESC_LEN_SIZE - 1 : 0]          m_desc_pk_len,
    output reg [`RL_DESC_APP_ID_SIZE -1 : 0]          m_desc_app_id
);

localparam APP_COUNT =  (2**`APP_ID_WIDTH);

// action table
// {activate, prio, id}
reg  [`CPU_MSG_APP_PRIO_SIZE -1 : 0] per_app_action_state [APP_COUNT-1 : 0];

// match port
reg  [15 : 0] per_app_match_state [APP_COUNT-1 : 0];


integer i, j;

initial begin
    for (i = 0; i < APP_COUNT; i = i + 1) begin
        per_app_match_state[i] = {16{1'b0}};
        per_app_action_state[i] = {`CPU_MSG_APP_PRIO_SIZE{1'b0}};
    end
end

genvar  appid;
generate
    for (appid=0; appid<APP_COUNT; appid = appid + 1) begin: appmatchaction
       always @(posedge clk) begin
            if( app_config_msg_en && appid == app_config_msg_app_id) begin
                per_app_match_state[appid] <= app_config_msg_port;
                per_app_action_state[appid] <= {app_config_msg_app_prio};
            end
       end
    end
endgenerate

/*
TCP/UDP Frame (IPv4)
 Field                       Length
 Destination MAC address     6 octets
 Source MAC address          6 octets
 Ethertype (0x0800)          2 octets
 Version (4)                 4 bits
 IHL (5-15)                  4 bits
 DSCP (0)                    6 bits
 ECN (0)                     2 bits
 length                      2 octets
 identification (0?)         2 octets
 flags (010)                 3 bits
 fragment offset (0)         13 bits
 time to live (64?)          1 octet
 protocol (6 or 17)          1 octet
 header checksum             2 octets
 source IP                   4 octets
 destination IP              4 octets
 options                     (IHL-5)*4 octets
 source port                 2 octets
 desination port             2 octets
 other fields + payload
TCP/UDP Frame (IPv6)
 Field                       Length
 Destination MAC address     6 octets
 Source MAC address          6 octets
 Ethertype (0x86dd)          2 octets
 Version (4)                 4 bits
 Traffic class               8 bits
 Flow label                  20 bits
 length                      2 octets
 next header (6 or 17)       1 octet
 hop limit                   1 octet
 source IP                   16 octets
 destination IP              16 octets
 source port                 2 octets
 desination port             2 octets
 other fields + payload
*/
parameter CYCLE_COUNT = (38+KEEP_WIDTH-1)/KEEP_WIDTH;


reg active_reg = 1'b1;


reg [15:0] eth_type_reg = 15'd0, eth_type_next;
reg [3:0] ihl_reg = 4'd0, ihl_next;

reg [4*8-1:0] ip_src_reg = 32'h0, ip_src_next;
reg [4*8-1:0] ip_dest_reg = 32'h0, ip_dest_next;
reg [4*8-1:0] ip_len_reg = 32'h0, ip_len_next;
reg [2*8-1:0] port_src_reg = 16'h0, port_src_next;
reg [2*8-1:0] port_dest_reg = 16'h0, port_dest_next;


reg ipv4_reg = 1'b0, ipv4_next;
reg tcp_reg = 1'b0, tcp_next;
reg udp_reg = 1'b0, udp_next;


always @(posedge clk) begin
    if(rst) begin
        active_reg <= 1;
    end
    else begin
        if(s_axis_tlast && s_axis_tvalid) begin
            active_reg <= 1;
        end
        else if(s_axis_tvalid && active_reg) begin
            active_reg <= 0;
        end
    end
end


always @* begin
    eth_type_next = eth_type_reg;
    ipv4_next = ipv4_reg;
    tcp_next = tcp_reg;
    udp_next = udp_reg;
    
    port_src_next = port_src_reg;
    port_dest_next = port_dest_reg;
    ip_src_next   = ip_src_reg;
    ip_dest_next   = ip_dest_reg;
    ip_len_next   = ip_len_reg;
    ihl_next = ihl_reg;

    
    if(s_axis_tvalid && active_reg) begin
        eth_type_next = 1'b0;
        ipv4_next = 1'b0;
        tcp_next = 1'b0;
        udp_next = 1'b0;
        
        port_src_next = 0;
        port_dest_next = 0;
        ip_src_next   = 0;
        ip_dest_next   = 0;
        ip_len_next = 0;
        ihl_next = 0;

        eth_type_next[15:8] = s_axis_tdata[(12%KEEP_WIDTH)*8 +: 8];
        eth_type_next[7:0] = s_axis_tdata[(13%KEEP_WIDTH)*8 +: 8];
        if (eth_type_next == 16'h0800) begin
            // ipv4
            ipv4_next = 1'b1;
        end 
        ihl_next = s_axis_tdata[(14%KEEP_WIDTH)*8 +: 8];
        if (ipv4_next) begin

            ip_len_next[15:8] = s_axis_tdata[(16%KEEP_WIDTH)*8 +: 8];
            ip_len_next[7:0]  = s_axis_tdata[(17%KEEP_WIDTH)*8 +: 8];

            if (s_axis_tdata[(23%KEEP_WIDTH)*8 +: 8] == 8'h06 && ihl_next == 5) begin
                // TCP
                tcp_next = 1'b1;
            end else if (s_axis_tdata[(23%KEEP_WIDTH)*8 +: 8] == 8'h11 && ihl_next == 5) begin
                // UDP
                udp_next = 1'b1;
            end
            ip_src_next[31:24] = s_axis_tdata[(26%KEEP_WIDTH)*8 +: 8];
            ip_src_next[23:16] = s_axis_tdata[(27%KEEP_WIDTH)*8 +: 8];
            ip_src_next[15:8] = s_axis_tdata[(28%KEEP_WIDTH)*8 +: 8];
            ip_src_next[7:0] = s_axis_tdata[(29%KEEP_WIDTH)*8 +: 8];

            ip_dest_next[31:24] = s_axis_tdata[(30%KEEP_WIDTH)*8 +: 8];
            ip_dest_next[23:16] = s_axis_tdata[(31%KEEP_WIDTH)*8 +: 8];
            ip_dest_next[15:8] = s_axis_tdata[(32%KEEP_WIDTH)*8 +: 8];
            ip_dest_next[7:0] = s_axis_tdata[(33%KEEP_WIDTH)*8 +: 8];

            if (tcp_next || udp_next) begin
               port_src_next[15:8] = s_axis_tdata[(34%KEEP_WIDTH)*8 +: 8];
               port_src_next[7:0] = s_axis_tdata[(35%KEEP_WIDTH)*8 +: 8];
               port_dest_next[15:8] = s_axis_tdata[(36%KEEP_WIDTH)*8 +: 8];
               port_dest_next[7:0] = s_axis_tdata[(37%KEEP_WIDTH)*8 +: 8];
            end

        end
    end
end


reg [APP_COUNT-1 : 0] if_match ;
wire  [`APP_ID_WIDTH-1 : 0] if_match_index ;
wire   if_match_index_valid ;
genvar  matchid;
generate
    for (matchid=0; matchid<APP_COUNT; matchid = matchid + 1) begin: findport
       always @(*) begin
           if_match[matchid] = 0;
           if(per_app_match_state[matchid] == port_dest_next && udp_next == 1'b1) begin
               if_match[matchid] = 1;
           end
       end
    end
endgenerate


priority_encoder #(
    .WIDTH(APP_COUNT),
    .LSB_HIGH_PRIORITY(0)
)
priority_encoder_masked (
    .input_unencoded(if_match),
    .output_valid(if_match_index_valid),
    .output_encoded(if_match_index),
    .output_unencoded()
);

always @*begin
    m_desc_pk_len = ip_len_next + 14; // add eth packet header
    m_desc_app_id = if_match_index_valid ? if_match_index : 0;
    m_desc_prio = per_app_action_state[if_match_index];
end

always @(posedge clk) begin

    if(rst) begin
        eth_type_reg <= 0;
        ihl_reg <= 0;
        ipv4_reg <= 0;
        tcp_reg <= 0;
        udp_reg <= 0;
        ip_src_reg <= 0;
        ip_dest_reg <= 0;
        ip_len_reg <= 0;
        port_src_reg <= 0;
        port_dest_reg <= 0;
    end
    else begin
        eth_type_reg <= eth_type_next;
        ihl_reg <= ihl_next;

        ipv4_reg <= ipv4_next;
        tcp_reg <= tcp_next;
        udp_reg <= udp_next;

        ip_src_reg <= ip_src_next;
        ip_dest_reg <= ip_dest_next;
        ip_len_reg <= ip_len_next;
        port_src_reg <= port_src_next;
        port_dest_reg <= port_dest_next;
    end

end

// ila_0 parser_debug (
// 	.clk(clk), // input wire clk

// 	.probe0(s_axis_tvalid), // input wire [0:0] probe0  
// 	.probe1({m_desc_app_id, m_desc_prio, if_match, port_dest_next, app_config_msg_app_id, app_config_msg_port, app_config_msg_app_prio, app_config_msg_en})
// );


endmodule
