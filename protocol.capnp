@0x8f2371dbd372cf8c;

struct Time {
    nseconds   @0   : UInt64;
}

struct SocketAddr {
    ip         @0   : UInt32;  # TODO: supporting IP V6
    port       @1   : UInt16;
}

struct Hash {
    data        @0  : Data;
}

struct Connect {
    address     @0  : SocketAddr;
    time        @1  : Time;
}

struct Propose {
    height      @0  : UInt64;
    round       @1  : UInt32;
    time        @2  : Time;
    prevHash    @3  : Hash;
}

struct Prevote {
    height      @0  : UInt64;
    round       @1  : UInt64;
    blockHash   @2  : Hash;
}

struct Precommit {
    height      @0  : UInt64;
    round       @1  : UInt64;
    blockHash   @2  : Hash;
}

struct Commit {
    height      @0  : UInt64;
    blockHash   @1  : Hash;
}
