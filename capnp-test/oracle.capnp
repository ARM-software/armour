@0xee42abf0e4dbeb2b;

interface Oracle {
  struct Value {
    union {
      bool @0 :Bool;
      int64 @1 :Int64;
      float64 @2 :Float64;
      text @3 :Text;
      data @4 :Data;
    }
  }
  struct Call {
    method @0 :Text;
    args @1 :List(Value);
  }
  eval @0 (calls :List(Call)) -> (results: List(Value));
  update @1 (calls :List(Call)) -> ();
}
