// The T3 Code revision ot3 is validated against (upstream/t3code.json), for every generator.
const fs=require('fs'),path=require('path');
const file=path.join(__dirname,'../upstream/t3code.json');
const pin=JSON.parse(fs.readFileSync(file,'utf8'));
if(!/^[0-9a-f]{40}$/.test(pin.revision))throw new Error('upstream/t3code.json: revision must be a full commit SHA');
module.exports=pin;
