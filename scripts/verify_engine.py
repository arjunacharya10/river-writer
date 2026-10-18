#!/usr/bin/env python3
"""Export original fixtures and compare independent Poppler renders to Rust preview.
Requires an already built target/debug/river (or RIVER_BIN), Poppler, Pillow, NumPy.
"""
import json
import os
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
from PIL import Image, ImageFilter
import numpy as np

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'output' / 'engine-verification'
BIN = os.environ.get('RIVER_BIN', str(ROOT / 'target/debug/river'))

def dilate(mask):
    return np.asarray(Image.fromarray(mask).filter(ImageFilter.MaxFilter(3)))

def compare(a, b):
    a = np.asarray(Image.open(a).convert('L'))
    b = np.asarray(Image.open(b).convert('L'))
    assert a.shape == b.shape == (1584, 1224)
    ink_a, ink_b = a < 192, b < 192
    assert min(ink_a.sum(), ink_b.sum()) > 20
    unmatched = int((ink_a & ~dilate(ink_b)).sum() + (ink_b & ~dilate(ink_a)).sum())
    mean = float(np.abs(a.astype(float) - b).mean())
    # Different rasterizers can disagree at a sharp corner/underline junction.
    # No translation/alignment is fitted; tolerate at most 0.1% of ink pixels.
    fraction = unmatched / max(int(ink_a.sum() + ink_b.sum()), 1)
    assert fraction <= .001, f'Geometry mismatch: {fraction:.4%} of ink unmatched'
    assert mean < 2.0, f'Excess darkness difference: {mean}'
    return {'unmatched_ink_fraction_1px': fraction, 'mean_absolute_difference_0_255': mean}

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    report = {}
    for template in ['screenplay', 'screenplay-multicam', 'stageplay', 'manuscript']:
        source = ROOT / 'fixtures' / ('manuscript.md' if template == 'manuscript' else 'screenplay.fountain')
        pdf = OUT / f'{template}.pdf'
        preview = OUT / template
        layout = OUT / f'{template}.json'
        subprocess.run([BIN, 'export', str(source), '--template', template,
                        '-o', str(pdf), '--preview-dir', str(preview),
                        '--layout-json', str(layout), '--force'], check=True)
        pages = json.loads(layout.read_text())['pages']
        subprocess.run(['pdftoppm', '-r', '144', '-png', str(pdf), str(OUT / f'{template}-pdf')], check=True)
        rendered = sorted(OUT.glob(f'{template}-pdf-*.png'))
        assert len(rendered) == len(pages)
        results = [compare(preview / f'page-{i:03}.png', path) for i, path in enumerate(rendered, 1)]
        text = ' '.join(subprocess.check_output(['pdftotext', '-layout', str(pdf), '-'], text=True).split())
        for snippet in ['The Other Bank', 'N. Writer']:
            assert snippet in text, f'Missing title-page text: {snippet}'
        assert ('Nora' if template == 'manuscript' else 'MAYA') in text
        for hidden in ['discarded paragraph', 'two lines above overlap', 'Check the season']:
            assert hidden not in text
        fonts = subprocess.check_output(['pdffonts', str(pdf)], text=True)
        assert 'CourierPrime' in fonts
        font_rows = fonts.splitlines()[2:]
        assert all('yes' in row for row in font_rows), fonts
        # Confirm the non-title screenplay scene heading sits at 1.5 inches.
        if template == 'screenplay':
            bbox = ET.fromstring(subprocess.check_output(['pdftotext', '-bbox', str(pdf), '-']))
            words = [e for e in bbox.iter() if e.tag.endswith('word')]
            ext = next(e for e in words if e.text == 'EXT.')
            assert abs(float(ext.attrib['xMin']) - 108) < .02
        report[template] = {'pages': len(pages), 'comparisons': results,
                            'selectable_text': True, 'bundled_fonts': True}
    # Deliberately shifted preview must fail; otherwise a loose comparator could
    # falsely approve geometry drift.
    original = OUT/'screenplay'/'page-002.png'
    image = Image.open(original).convert('L')
    shifted = Image.new('L', image.size, 255)
    shifted.paste(image, (3, 0))
    negative = OUT/'negative-control.png'
    shifted.save(negative)
    try:
        compare(original, negative)
    except AssertionError:
        report['negative_control_3px_shift'] = 'rejected'
    else:
        raise AssertionError('Comparator accepted a 3px shift')
    (OUT/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps(report, indent=2))

if __name__ == '__main__':
    main()
