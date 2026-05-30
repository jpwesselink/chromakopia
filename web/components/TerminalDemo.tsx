import React, { useCallback, useEffect, useRef, useState } from 'react';

const COLS_CAROUSEL = 95;
const FRAME_MS = 1000 / 30;
const CHAR_WIDTH_PX = 7.8; // approximate xterm monospace char width at fontSize 13

function computeBareCols(): number {
  if (typeof window === 'undefined') return 240;
  return Math.max(120, Math.ceil(window.innerWidth / CHAR_WIDTH_PX));
}

interface DemoInfo {
  id: number;
  name: string;
  count: number;
}

// Stable reference to loaded modules — never changes after first load
interface Modules {
  wasm: { make_renderer: Function; demo_count: () => number; demo_name: (id: number) => string };
  Terminal: typeof import('@xterm/xterm').Terminal;
}

export function TerminalDemo({ bare = false }: { bare?: boolean } = {}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const modulesRef = useRef<Modules | null>(null);
  const termRef = useRef<InstanceType<typeof import('@xterm/xterm').Terminal> | null>(null);
  const rendererRef = useRef<any>(null);
  const frameRef = useRef(0);
  const rafRef = useRef(0);
  const lastTickRef = useRef(0);
  const currentIdRef = useRef(0);
  const intervalRef = useRef<ReturnType<typeof setInterval>>();

  const [demoInfo, setDemoInfo] = useState<DemoInfo | null>(null);

  const cols = bare ? computeBareCols() : COLS_CAROUSEL;

  const runAnimation = useCallback(() => {
    cancelAnimationFrame(rafRef.current);

    function tick(ts: number) {
      if (ts - lastTickRef.current >= FRAME_MS) {
        lastTickRef.current = ts;
        if (rendererRef.current && termRef.current) {
          try {
            const ansi = rendererRef.current.frame(frameRef.current++).replace(/\n/g, '\r\n');
            termRef.current.write('\x1b[H' + ansi);
          } catch (err) {
            console.warn('[TerminalDemo] frame error, stopping tick:', err);
            return;
          }
        }
      }
      rafRef.current = requestAnimationFrame(tick);
    }

    rafRef.current = requestAnimationFrame(tick);
  }, []);

  const switchTo = useCallback((id: number) => {
    const mods = modulesRef.current;
    if (!mods) return;

    const prev = rendererRef.current;
    rendererRef.current = null;
    try { prev?.free?.(); } catch {}

    const renderer = mods.wasm.make_renderer(id, cols);
    rendererRef.current = renderer;
    const rows = renderer.height();

    currentIdRef.current = id;
    frameRef.current = 0;

    if (!termRef.current) {
      const term = new mods.Terminal({
        cols,
        rows,
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', Consolas, monospace",
        theme: {
          background: '#0a0a0a',
          foreground: '#cccccc',
          cursor: '#0a0a0a',
          cursorAccent: '#0a0a0a',
        },
        cursorBlink: false,
        cursorStyle: 'bar' as const,
        scrollback: 0,
        disableStdin: true,
      });
      termRef.current = term;
      if (containerRef.current) {
        term.open(containerRef.current);
        containerRef.current.querySelector('textarea')?.setAttribute('autocomplete', 'off');
      }
    } else {
      termRef.current.resize(cols, rows);
      termRef.current.write('\x1b[2J\x1b[H');
    }

    setDemoInfo({
      id,
      name: mods.wasm.demo_name(id),
      count: mods.wasm.demo_count(),
    });

    runAnimation();
  }, [runAnimation, cols]);

  useEffect(() => {
    let cancelled = false;

    Promise.all([
      import('../wasm/chromakopia_wasm.js'),
      import('@xterm/xterm'),
      import('@xterm/xterm/css/xterm.css'),
    ]).then(async ([wasm, xterm]) => {
      if (cancelled) return;
      // --target web wasm-pack: must initialize before any export is callable.
      await (wasm as any).default();
      if (cancelled) return;
      modulesRef.current = { wasm, Terminal: xterm.Terminal };
      switchTo(0); // Showcase (letters_drop)

      // Auto-cycle every 30 s — only in the full carousel instance.
      // Bare mode (hero at top of page) stays pinned on the showcase.
      if (!bare) {
        intervalRef.current = setInterval(() => {
          const count = modulesRef.current?.wasm.demo_count() ?? 1;
          const next = (currentIdRef.current + 1) % count;
          switchTo(next);
        }, 30_000);
      }
    });

    return () => {
      cancelled = true;
      clearInterval(intervalRef.current);
      cancelAnimationFrame(rafRef.current);
      try { rendererRef.current?.free?.(); } catch {}
      rendererRef.current = null;
      try { termRef.current?.dispose(); } catch {}
      termRef.current = null;
      modulesRef.current = null;
    };
  }, [switchTo, bare]);

  const handlePrev = useCallback(() => {
    const count = modulesRef.current?.wasm.demo_count() ?? 1;
    switchTo(((currentIdRef.current - 1) + count) % count);
  }, [switchTo]);

  const handleNext = useCallback(() => {
    const count = modulesRef.current?.wasm.demo_count() ?? 1;
    switchTo((currentIdRef.current + 1) % count);
  }, [switchTo]);

  const handleDot = useCallback((i: number) => () => switchTo(i), [switchTo]);

  if (bare) {
    return <div className="terminal-bare" ref={containerRef} />;
  }

  return (
    <div className="terminal-window">
      <div className="terminal-titlebar">
        <span className="terminal-dot red" />
        <span className="terminal-dot yellow" />
        <span className="terminal-dot green" />
        <span className="terminal-title">
          {demoInfo ? demoInfo.name : 'loading…'}
        </span>
      </div>
      <div className="terminal-body">
        <div id="xterm-container" ref={containerRef} />
      </div>
      {demoInfo && (
        <div className="demo-controls">
          <span className="demo-name">{demoInfo.name}</span>
          <div className="demo-nav">
            <div className="demo-dots">
              {Array.from({ length: demoInfo.count }, (_, i) => (
                <span
                  key={i}
                  className={`demo-dot${demoInfo.id === i ? ' active' : ''}`}
                  onClick={handleDot(i)}
                />
              ))}
            </div>
            <button onClick={handlePrev}>‹</button>
            <button onClick={handleNext}>›</button>
          </div>
        </div>
      )}
    </div>
  );
}
