import { core } from "ext:core/mod.js";
const ops = core.ops;

function myFn() {
  return ops.op_my_fn();
}

globalThis.MyExtension = { myFn };
