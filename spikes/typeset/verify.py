#!/usr/bin/env python3
"""Independent PDF/preview check. Requires Poppler, Pillow and NumPy."""
import json
from pathlib import Path
import subprocess
from PIL import Image, ImageFilter
import numpy as np

OUTPUT = Path(__file__).resolve().parent / "output"
EXPECTED = {
    "screenplay": ["EXT. FERRY TERMINAL - NIGHT", "Rain taps the roof", "The last boat left an hour ago.", "Somebody has been here."],
    "manuscript": ["RIVER / THE OTHER BANK / 1", "CHAPTER ONE", "By morning the river had carried away the steps.", "Come before the first frost."],
}

def compare(preview, rendered):
    a = np.array(Image.open(preview).convert("L"))
    b = np.array(Image.open(rendered).convert("L"))
    assert a.shape == b.shape == (1584, 1224), "Expected Letter at 144 dpi"
    ink_a, ink_b = a < 192, b < 192
    assert ink_a.sum() > 10000 and ink_b.sum() > 10000, "Blank/near-blank output"
    def dilate(mask):
        return np.array(Image.fromarray(mask).filter(ImageFilter.MaxFilter(3)))
    # Each dark pixel must have a dark counterpart within a one-pixel
    # Chebyshev radius. No alignment/translation is fitted to either image.
    unmatched_a = int((ink_a & ~dilate(ink_b)).sum())
    unmatched_b = int((ink_b & ~dilate(ink_a)).sum())
    mae = float(np.abs(a.astype(float)-b).mean())
    ink_ratio = float(ink_b.sum()/ink_a.sum())
    assert unmatched_a == unmatched_b == 0, (unmatched_a, unmatched_b)
    assert mae < 1.5, f"Excess image difference: {mae}"
    assert 0.85 < ink_ratio < 1.15, f"Excess darkness difference: {ink_ratio}"
    return {"size_px": [1224,1584], "unmatched_preview_pixels": unmatched_a,
            "unmatched_pdf_pixels": unmatched_b, "mean_absolute_difference_0_255": mae,
            "pdf_to_preview_ink_ratio": ink_ratio}

def main():
    results = {}
    for name, snippets in EXPECTED.items():
        pdf = OUTPUT / f"{name}.pdf"
        info = subprocess.check_output(["pdfinfo", str(pdf)], text=True)
        assert "Pages:           1" in info
        subprocess.run(["pdftoppm", "-r", "144", "-png", "-singlefile",
                        "-aa", "yes", "-aaVector", "yes", str(pdf),
                        str(OUTPUT/f"{name}-pdf")], check=True)
        text = " ".join(subprocess.check_output(["pdftotext", "-layout", str(pdf), "-"], text=True).split())
        for snippet in snippets:
            assert snippet in text, f"Missing selectable text: {snippet}"
        results[name] = compare(OUTPUT/f"{name}.png", OUTPUT/f"{name}-pdf.png")
    # Confirm the comparator rejects a genuine 3px positioning regression.
    a = Image.open(OUTPUT/"screenplay.png").convert("L")
    shifted = Image.new("L", a.size, 255)
    shifted.paste(a, (3,0))
    shifted.save(OUTPUT/"shifted-negative-control.png")
    try:
        compare(OUTPUT/"screenplay.png", OUTPUT/"shifted-negative-control.png")
    except AssertionError:
        results["negative_control_3px_shift"] = "correctly rejected"
    else:
        raise AssertionError("Comparator failed to reject shifted output")
    (OUTPUT/"verification.json").write_text(json.dumps(results, indent=2)+"\n")
    print(json.dumps(results, indent=2))

if __name__ == "__main__":
    main()
