/** Cria um elemento. Atributos: class, on<evento>, dataset, e qualquer atributo HTML. Filhos: nós, textos ou listas. */
export function h(tag, atributos = {}, ...filhos) {
  const el = document.createElement(tag);
  for (const [chave, valor] of Object.entries(atributos || {})) {
    if (valor === null || valor === undefined || valor === false) continue;
    if (chave === 'class') el.className = valor;
    else if (chave === 'dataset') Object.assign(el.dataset, valor);
    else if (chave.startsWith('on') && typeof valor === 'function') {
      el.addEventListener(chave.slice(2).toLowerCase(), valor);
    } else el.setAttribute(chave, valor === true ? '' : String(valor));
  }
  for (const filho of filhos.flat(Infinity)) {
    if (filho === null || filho === undefined || filho === false) continue;
    el.append(filho instanceof Node ? filho : document.createTextNode(String(filho)));
  }
  return el;
}

export const $ = (seletor, raiz = document) => raiz.querySelector(seletor);

export function limpar(el) {
  el.replaceChildren();
  return el;
}

/** O SQLite guarda CURRENT_TIMESTAMP em UTC, sem fuso: "2026-10-06 14:03:00". */
function lerDataUtc(texto) {
  if (!texto) return null;
  const d = new Date(String(texto).replace(' ', 'T') + 'Z');
  return Number.isNaN(d.getTime()) ? null : d;
}

export function formatarDataHora(texto) {
  const d = lerDataUtc(texto);
  return d ? d.toLocaleString('pt-BR', { dateStyle: 'short', timeStyle: 'short' }) : '';
}

export function formatarDataLonga(texto) {
  const d = lerDataUtc(texto);
  return d ? d.toLocaleString('pt-BR', { dateStyle: 'full', timeStyle: 'short' }) : '';
}

export function formatarDuracao(segundos) {
  if (segundos === null || segundos === undefined) return '';
  const h = Math.floor(segundos / 3600);
  const m = Math.floor((segundos % 3600) / 60);
  const s = segundos % 60;
  if (h > 0) return `${h} h ${String(m).padStart(2, '0')} min`;
  if (m > 0) return `${m} min`;
  return `${s} s`;
}

export function formatarRelogio(segundos) {
  const h = Math.floor(segundos / 3600);
  const m = Math.floor((segundos % 3600) / 60);
  const s = Math.floor(segundos % 60);
  const mm = String(m).padStart(2, '0');
  const ss = String(s).padStart(2, '0');
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

/** Aviso temporário no canto da tela. tipo: info | ok | erro */
export function aviso(texto, tipo = 'info') {
  const caixa = $('#avisos');
  const el = h('div', { class: 'aviso', dataset: { tipo } }, texto);
  caixa.append(el);
  setTimeout(() => el.remove(), tipo === 'erro' ? 9000 : 5000);
}

/** Confirmação própria (window.confirm não é confiável no webview do Linux). Resolve true/false. */
export function confirmar({ titulo, texto, acao = 'Confirmar', perigo = false }) {
  const dlg = $('#dlg-confirmar');
  $('[data-campo="titulo"]', dlg).textContent = titulo;
  $('[data-campo="texto"]', dlg).textContent = texto;
  const botao = $('[data-acao="confirmar"]', dlg);
  botao.textContent = acao;
  botao.classList.toggle('btn-perigo', perigo);
  botao.classList.toggle('btn-primario', !perigo);
  return new Promise((resolve) => {
    dlg.returnValue = '';
    dlg.addEventListener('close', () => resolve(dlg.returnValue === 'confirmar'), { once: true });
    dlg.showModal();
  });
}
