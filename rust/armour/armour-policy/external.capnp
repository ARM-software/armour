@0xd2d288d44efc3384;

interface External {
  struct Value {
    union {
      bool @0 :Bool;
      int64 @1 :Int64;
      float64 @2 :Float64;
      text @3 :Text;
      data @4 :Data;
      unit @5 :Void;
      tuple @6 :List(Value);
      list @7 :List(Value);
    }
  }
  call @0 (name :Text, args :List(Value)) -> (result :Value);
}
