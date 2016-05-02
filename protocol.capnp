@0x8f2371dbd372cf8c;

struct Connect {
    ip         @0   : UInt32;  # TODO: supporting IP V6
    port       @1   : UInt16;
    time       @2   : UInt64;
}

struct Propose {
    height      @0  : UInt64;
    round       @1  : UInt32;
    time        @2  : UInt64;
    prevHash    @3  : Data;
}

struct Prevote {
    height      @0  : UInt64;
    round       @1  : UInt64;
    blockHash   @2  : Data;
}

struct Precommit {
    height      @0  : UInt64;
    round       @1  : UInt64;
    blockHash   @2  : Data;
}

struct Commit {
    height      @0  : UInt64;
    blockHash   @1  : Data;
}
