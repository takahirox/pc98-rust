// Collect only the owned emulator Worker's host CPU profile after an audio
// assertion fails. Profiling never runs inside a normal acceptance window.
export async function workerProfile(browser, milliseconds = 2000) {
  const session = await browser.newBrowserCDPSession();
  let workerSession;
  const pending = new Map();
  let nextId = 0;
  try {
    const {targetInfos} = await session.send('Target.getTargets');
    const target = targetInfos.find(t => t.type === 'worker' && t.url.endsWith('/emulator-worker.js'));
    if (!target) throw Error('Emulator Worker target missing');
    ({sessionId:workerSession} = await session.send('Target.attachToTarget', {targetId:target.targetId, flatten:false}));
    session.on('Target.receivedMessageFromTarget', event => {
      if (event.sessionId !== workerSession) return;
      const message = JSON.parse(event.message), waiter = pending.get(message.id);
      if (!waiter) return;
      pending.delete(message.id);
      if (message.error) waiter.reject(Error(message.error.message));
      else waiter.resolve(message.result);
    });
    function call(method) {
      const id = ++nextId;
      return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => { pending.delete(id); reject(Error('Worker profiler timed out')); }, 10000);
        const waiter = {resolve:value => { clearTimeout(timeout); resolve(value); }, reject:error => { clearTimeout(timeout); reject(error); }};
        pending.set(id, waiter);
        session.send('Target.sendMessageToTarget', {sessionId:workerSession, message:JSON.stringify({id,method})})
          .catch(error => { pending.delete(id); waiter.reject(error); });
      });
    }
    await call('Profiler.enable');
    await call('Profiler.start');
    await new Promise(resolve => setTimeout(resolve, milliseconds));
    return (await call('Profiler.stop')).profile;
  } finally {
    for (const waiter of pending.values()) waiter.reject(Error('Worker profiler closed'));
    if (workerSession) await session.send('Target.detachFromTarget', {sessionId:workerSession}).catch(() => {});
    await session.detach().catch(() => {});
  }
}
