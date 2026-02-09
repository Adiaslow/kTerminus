import { describe, it, expect } from "vitest";
import { render } from "../test/utils";
import {
  PlusIcon,
  XIcon,
  MenuIcon,
  TerminalIcon,
  DocumentIcon,
  CheckIcon,
  AlertTriangleIcon,
  InfoCircleIcon,
  OrchestratorIcon,
} from "./Icons";

describe("Icons", () => {
  it("renders PlusIcon with custom className", () => {
    render(<PlusIcon className="w-4 h-4 text-red-500" />);
    const svg = document.querySelector("svg");
    expect(svg).toHaveClass("w-4", "h-4", "text-red-500");
  });

  it("renders XIcon", () => {
    render(<XIcon data-testid="x-icon" />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders MenuIcon", () => {
    render(<MenuIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders TerminalIcon", () => {
    render(<TerminalIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders DocumentIcon", () => {
    render(<DocumentIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders CheckIcon", () => {
    render(<CheckIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders AlertTriangleIcon", () => {
    render(<AlertTriangleIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders InfoCircleIcon", () => {
    render(<InfoCircleIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("renders OrchestratorIcon", () => {
    render(<OrchestratorIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("icons have aria-hidden by default", () => {
    render(<PlusIcon />);
    const svg = document.querySelector("svg");
    expect(svg).toHaveAttribute("aria-hidden", "true");
  });

  it("icons support fill prop", () => {
    render(<PlusIcon fill="currentColor" />);
    const svg = document.querySelector("svg");
    expect(svg).toHaveAttribute("fill", "currentColor");
  });
});
