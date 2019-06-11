@0xd2d288d44efc3384;

interface External {
  struct Entry {
    key @0 :Text;
    value :union {
      data @1 :Data;
      text @2 :Text;
      unit @3 :Void;
    }
  }
  struct Value {
    union {
      bool @0 :Bool;
      int64 @1 :Int64;
      float64 @2 :Float64;
      text @3 :Text;
      data @4 :Data;
      unit @5 :Void;
      stringmap @6 :List(Entry);
    }
  }
  call @0 (name :Text, args :List(Value)) -> (result :Value);
}
