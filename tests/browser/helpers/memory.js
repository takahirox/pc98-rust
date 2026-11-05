import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

export async function trackMemory(browser) {
  const session = await browser.newBrowserCDPSession();
  let peakRendererRssKiB=0, peakBrowserRssKiB=0, sampling=false;
  let rendererRssKiB=0, browserRssKiB=0;
  const sample = async () => {
    if (sampling) return;
    sampling=true;
    try {
      const {processInfo}=await session.send('SystemInfo.getProcessInfo');
      const renderers=new Set(processInfo.filter(p=>p.type==='renderer').map(p=>p.id));
      const pids=processInfo.map(p=>String(p.id));
      if (pids.length) {
        const {stdout}=await promisify(execFile)('ps',['-o','pid=,rss=','-p',pids.join(',')]);
        let rendererRss=0, totalRss=0;
        for(const line of stdout.trim().split('\n')) {
          const [pid,rss]=line.trim().split(/\s+/).map(Number);
          totalRss+=rss;
          if(renderers.has(pid)) rendererRss+=rss;
        }
        rendererRssKiB=rendererRss; browserRssKiB=totalRss;
        peakRendererRssKiB=Math.max(peakRendererRssKiB,rendererRss);
        peakBrowserRssKiB=Math.max(peakBrowserRssKiB,totalRss);
      }
    } finally { sampling=false; }
  };
  // Sample all browser-reported process RSS as well as renderers, including
  // native download allocations; neither is an exact OS high-water measurement.
  // Summed RSS can count shared pages more than once.
  const timer=setInterval(()=>sample().catch(()=>{}),100);
  return {
    sample,
    current: () => ({rendererRssKiB, browserRssKiB}),
    peak: () => peakRendererRssKiB,
    totalPeak: () => peakBrowserRssKiB,
    reset: () => { peakRendererRssKiB=0; peakBrowserRssKiB=0; },
    stop: async () => { clearInterval(timer); await session.detach(); },
  };
}
