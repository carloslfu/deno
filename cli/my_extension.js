import * as ops from "ext:core/ops";

function myFn() {
  console.log("ops", ops);

  return ops.op_my_fn();
}

globalThis.MyExtension = { myFn };
