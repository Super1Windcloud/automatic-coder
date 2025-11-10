
import {createHash } from 'crypto'
const str="activation_status_fingerprint";

//不可逆
const hash = createHash("sha256")
  .update(str)
  .digest("hex");

console.log(hash);