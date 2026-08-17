import { useState } from "react";

function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

export function ThemeToggle() {
  const [dark, setDark] = useState(isDark);

  function toggle(): void {
    const next = !dark;
    document.documentElement.classList.toggle("dark", next);
    localStorage.setItem("ui-theme", next ? "dark" : "light");
    setDark(next);
  }

  return (
    <button
      onClick={toggle}
      className="btn btn-sm"
    >
      {dark ? "☀ Light" : "☾ Dark"}
    </button>
  );
}
