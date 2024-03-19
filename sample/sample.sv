// This is dummy circuits for testing svinst_port

// Case 1: ports are dedined IN the module definition
module case1 (
    input  logic        CLK, RST,
    input  logic [31:0] DATA_IN,
    output logic  [7:0] DATA_OUT,
    output logic        BUSY);

    logic busy_a, busy_b;
    assign BUSY = busy_a | busy_b;

    case2 c2a (CLK, RST, DATA_IN[15:0], DATA_OUT[3:0], busy_a);
    case2 c2b (CLK, RST, DATA_IN[31:16], DATA_OUT[7:4], busy_b);
endmodule

// Case 2: ports are dedined AFTER the module definition
module case2 (CLK, RST, DIN, DOUT, BUSY);
    input  logic        CLK, RST;
    input  logic [15:0] DIN;
    output logic  [3:0] DOUT;
    output logic        BUSY;

    assign DOUT = 4'b0000;
    assign BUSY = 1'b0;
endmodule
