@0xff3554128c156245;


struct UiViewMetadata {
  title @0 :Text;
  dateSaved @1 :UInt64; # milliseconds since unix epoch
  addedBy @2 :Text; # Identity ID, encoded in hexadecimal format.
}
