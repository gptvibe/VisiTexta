import React from "react";

type GlowCardProps = {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
  className?: string;
};

export default function GlowCard({ title, subtitle, children, className }: GlowCardProps) {
  return (
    <section
      className={`glass-panel relative overflow-hidden rounded-3xl px-6 py-6 ${
        className ? className : ""
      }`}
    >
      <div className="pointer-events-none absolute inset-0 opacity-40">
        <div className="absolute -left-12 -top-10 h-32 w-32 rounded-full bg-neon-500/40 blur-3xl" />
        <div className="absolute -right-10 bottom-0 h-28 w-28 rounded-full bg-lava-500/40 blur-3xl" />
      </div>
      <div className="relative z-10">
        <div className="mb-4">
          <h2 className="text-lg font-semibold text-white">{title}</h2>
          {subtitle ? <p className="text-sm text-white/60">{subtitle}</p> : null}
        </div>
        {children}
      </div>
    </section>
  );
}
