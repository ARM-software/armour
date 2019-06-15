@0xd3bfe2233e2c3c1f;

interface Docker {
    listen @0 () -> ();
    createNetwork @1 (network: Text) -> (result: Bool);
    removeNetwork @2 (network: Text) -> (result: Bool);
    attachToNetwork @3 (container: Text, network: Text) -> (result: Bool);
    detachFromNetwork @4 (container: Text, network: Text) -> (result: Bool);
}
