import {
  Chart,
  LineController,
  LineElement,
  PointElement,
  LinearScale,
  TimeScale,
  Tooltip,
  Legend,
  type ChartConfiguration,
} from "chart.js";
import "chartjs-adapter-date-fns";
import type { IlPoint } from "./api";

Chart.register(LineController, LineElement, PointElement, LinearScale, TimeScale, Tooltip, Legend);

// Chart.js renders text on canvas, not the DOM, so it doesn't inherit the
// page's CSS font — set it explicitly to match.
Chart.defaults.font.family =
  "'Inter Variable', ui-sans-serif, system-ui, -apple-system, 'Segoe UI', sans-serif";

let chart: Chart | null = null;

const currencyFmt = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

export function renderIlChart(canvas: HTMLCanvasElement, points: IlPoint[]): void {
  const labels = points.map((p) => p.timestamp * 1000);

  const config: ChartConfiguration<"line"> = {
    type: "line",
    data: {
      labels,
      datasets: [
        {
          label: "Hold value",
          data: points.map((p) => p.holdValue),
          borderColor: "#38bdf8",
          backgroundColor: "#38bdf8",
          yAxisID: "value",
          pointRadius: 0,
          tension: 0.15,
        },
        {
          label: "LP value",
          data: points.map((p) => p.lpValue),
          borderColor: "#a78bfa",
          backgroundColor: "#a78bfa",
          yAxisID: "value",
          pointRadius: 0,
          tension: 0.15,
        },
        {
          label: "IL %",
          data: points.map((p) => p.ilPercent),
          borderColor: "#fb7185",
          backgroundColor: "#fb7185",
          yAxisID: "ilPercent",
          pointRadius: 0,
          tension: 0.15,
          borderDash: [4, 3],
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: "index", intersect: false },
      scales: {
        x: {
          type: "time",
          time: { unit: "day" },
          ticks: { color: "#94a3b8" },
          grid: { color: "#1e293b" },
        },
        value: {
          position: "left",
          ticks: { color: "#94a3b8", callback: (v) => currencyFmt.format(Number(v)) },
          grid: { color: "#1e293b" },
        },
        ilPercent: {
          position: "right",
          ticks: { color: "#94a3b8", callback: (v) => `${v}%` },
          grid: { display: false },
        },
      },
      plugins: {
        legend: { labels: { color: "#e2e8f0" } },
        tooltip: {
          callbacks: {
            label: (ctx) => {
              const v = ctx.parsed.y;
              if (v === null) return "";
              return ctx.dataset.label === "IL %"
                ? `IL: ${v.toFixed(4)}%`
                : `${ctx.dataset.label}: ${currencyFmt.format(v)}`;
            },
          },
        },
      },
    },
  };

  if (chart) {
    chart.data = config.data;
    chart.update();
    return;
  }
  chart = new Chart(canvas, config);
}
