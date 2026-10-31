import init, { convert_png } from './pkg/png2iff_core.js';

const $ = (id) => document.getElementById(id);
const radio = (name) => document.querySelector(`input[name=${name}]:checked`).value;

let pngBytes = null;
let fileName = 'bild';
let result = null;
let timer = null;

function options() {
  return [
    radio('mode'),
    Number($('planes').value),
    $('autoPlanes').checked,
    $('ocs').checked,
    $('dither').checked,
    $('compress').checked,
    radio('transparency'),
    parseInt($('bg').value.slice(1), 16),
  ];
}

function updateControls() {
  const mode = radio('mode');
  const indexed = mode === 'indexed';
  $('planes').disabled = !indexed;
  $('autoPlanes').disabled = !indexed;
  $('dither').disabled = !(indexed || mode === 'ehb');
  $('ocs').disabled = !(indexed || mode === 'ehb');
  $('keyRow').querySelector('input').disabled = !indexed;
  if (!indexed && radio('transparency') === 'key') {
    document.querySelector('input[name=transparency][value=mask]').checked = true;
  }
  $('planesVal').textContent = $('planes').value;
}

function draw(canvas, w, h, rgba) {
  canvas.width = w;
  canvas.height = h;
  canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
}

function showError(msg) {
  $('error').textContent = msg;
}

function run() {
  if (!pngBytes) return;
  try {
    result?.free();
    result = convert_png(pngBytes, ...options());
    showError('');
    draw($('prev'), result.width(), result.height(), result.preview_rgba());
    $('info').textContent = `${result.width()}×${result.height()} Pixel, ${result.planes()} Bitplanes, ` +
      `${result.colors()} Farben, IFF-Größe ${result.iff().length} Bytes`;
    $('save').disabled = false;
  } catch (e) {
    result = null;
    $('save').disabled = true;
    $('info').textContent = '';
    showError(String(e?.message ?? e));
  }
}

function scheduleRun() {
  updateControls();
  clearTimeout(timer);
  timer = setTimeout(run, 50);
}

async function loadFile(file) {
  if (!file) return;
  fileName = file.name.replace(/\.[^.]*$/, '') || 'bild';
  pngBytes = new Uint8Array(await file.arrayBuffer());
  try {
    const bmp = await createImageBitmap(new Blob([pngBytes], { type: 'image/png' }));
    const c = $('orig');
    c.width = bmp.width;
    c.height = bmp.height;
    c.getContext('2d').drawImage(bmp, 0, 0);
    $('previews').hidden = false;
  } catch {
    showError('Die Datei konnte nicht als PNG gelesen werden.');
    return;
  }
  run();
}

function save() {
  if (!result) return;
  const blob = new Blob([result.iff()], { type: 'application/octet-stream' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${fileName}.iff`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10000);
}

await init();
document.querySelectorAll('aside input').forEach((el) => el.addEventListener('input', scheduleRun));
$('file').addEventListener('change', (e) => loadFile(e.target.files[0]));
$('save').addEventListener('click', save);
const drop = $('drop');
['dragenter', 'dragover'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.add('over'); }));
['dragleave', 'drop'].forEach((t) => drop.addEventListener(t, (e) => { e.preventDefault(); drop.classList.remove('over'); }));
drop.addEventListener('drop', (e) => loadFile(e.dataTransfer.files[0]));
updateControls();

if ('serviceWorker' in navigator) {
  navigator.serviceWorker.register('sw.js').catch(() => {});
}
