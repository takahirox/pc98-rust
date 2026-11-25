// Synchronous disk writes and flushes belong only to this short-lived Worker.
let access;
onmessage = async ({data}) => {
  try {
    switch (data.type) {
      case 'open':
        access = await data.handle.createSyncAccessHandle();
        access.truncate(0);
        break;
      case 'write':
        if (access.write(data.buffer, {at:data.at}) !== data.buffer.byteLength) {
          throw Error('Short write to browser HDD storage');
        }
        break;
      case 'finish':
        if (access.getSize() !== data.bytes) throw Error('Incomplete staged HDD');
        access.flush();
        access.close();
        access = undefined;
        break;
    }
    postMessage({});
  } catch (error) {
    access?.close();
    access = undefined;
    postMessage({error:error.message});
  }
};
