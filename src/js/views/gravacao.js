import { h, limpar, aviso, formatarRelogio } from '../dom.js';
import { api, aoEvento } from '../api.js';
import { estado, nomeDaDisciplina } from '../state.js';

const MAX_BARRAS = 240;
let niveis = [];
let relogio = null;
let refs = null;
let callbacks = { aoFinalizar: async () => {}, aoMudar: () => {} };

window.addEventListener('resize', () => desenhar());

/** Liga uma vez o evento de nível do microfone (vem do Rust ~10 vezes por segundo). */
export function registrarEventosGravacao() {
  return aoEvento('gravacao:nivel', (nivel) => {
    if (!estado.gravacao.ativa) return;
    niveis.push(nivel);
    if (niveis.length > MAX_BARRAS) niveis.shift();
    desenhar();
  });
}

export function montarGravacao(destino, { aoFinalizar, aoMudar }) {
  pararRelogio();
  callbacks = { aoFinalizar, aoMudar };

  const botao = h(
    'button',
    { class: 'btn-gravar', type: 'button', onclick: alternar },
    h('span', { class: 'miolo' }),
  );
  const status = h('div', { class: 'gravacao-status' });
  const sub = h('div', { class: 'gravacao-sub' });
  const canvas = h('canvas', { class: 'fita', 'aria-hidden': 'true' });
  const tempo = h('div', { class: 'tempo' }, '00:00');
  const raiz = h(
    'section',
    { class: 'painel-gravacao', 'aria-label': 'Gravação de aula' },
    botao,
    h('div', { class: 'gravacao-centro' }, status, sub, canvas),
    tempo,
  );
  const faixa = h('div');
  destino.append(faixa, raiz);
  refs = { raiz, botao, status, sub, canvas, tempo, faixa };

  atualizarAvisoAmbiente();
  renderGravacao();
  if (estado.gravacao.ativa) iniciarRelogio();
}

/** Mostra o que falta configurar (modelo do Whisper, chave do Gemini). */
export function atualizarAvisoAmbiente() {
  if (!refs) return;
  limpar(refs.faixa);
  refs.faixa.className = '';
  const a = estado.ambiente;
  if (!a) return;
  const itens = [];
  if (!a.modelo_whisper_ok) {
    itens.push(
      `Modelo do Whisper não encontrado em ${a.caminho_modelo}. Rode scripts/download-model.sh ou ajuste o nome em Configurações.`,
    );
  }
  if (!a.gemini_key_ok) {
    itens.push(
      'Chave do Gemini não configurada. A aula é transcrita, mas a ata só é gerada depois que você definir a chave em Configurações.',
    );
  }
  if (!itens.length) return;
  refs.faixa.className = 'faixa-aviso';
  refs.faixa.append(...itens.map((t) => h('p', {}, t)));
}

async function alternar() {
  refs.botao.disabled = true;
  try {
    if (!estado.gravacao.ativa) {
      if (!estado.disciplinaId) return;
      await api.gravacao.iniciar(estado.disciplinaId);
      niveis = [];
      estado.gravacao = { ativa: true, disciplinaId: estado.disciplinaId, inicio: Date.now() };
      iniciarRelogio();
      callbacks.aoMudar();
    } else {
      const disciplinaGravada = estado.gravacao.disciplinaId;
      const id = await api.gravacao.parar();
      estado.gravacao = { ativa: false, disciplinaId: null, inicio: null };
      pararRelogio();
      niveis = [];
      aviso('Gravação salva. A transcrição começou em segundo plano.', 'ok');
      callbacks.aoMudar();
      await callbacks.aoFinalizar(id, disciplinaGravada);
    }
  } catch (e) {
    // Se o Rust já encerrou a gravação (ex.: ela era curta demais), a interface acompanha.
    try {
      const rec = await api.gravacao.estado();
      if (!rec.gravando && estado.gravacao.ativa) {
        estado.gravacao = { ativa: false, disciplinaId: null, inicio: null };
        pararRelogio();
        callbacks.aoMudar();
      }
    } catch (_) { /* sem ação */ }
    aviso(String(e), 'erro');
  } finally {
    if (refs) refs.botao.disabled = false;
    renderGravacao();
  }
}

function renderGravacao() {
  if (!refs) return;
  const g = estado.gravacao;
  refs.raiz.dataset.ativa = String(g.ativa);
  refs.botao.setAttribute('aria-pressed', String(g.ativa));
  refs.botao.setAttribute('aria-label', g.ativa ? 'Parar gravação' : 'Gravar aula');
  if (g.ativa) {
    refs.status.textContent = `Gravando: ${nomeDaDisciplina(g.disciplinaId)}`;
    refs.sub.textContent = 'Clique no botão para parar e iniciar a transcrição.';
  } else {
    refs.status.textContent = `Gravar aula de ${nomeDaDisciplina(estado.disciplinaId)}`;
    refs.sub.textContent = 'O áudio fica no seu computador e a transcrição roda localmente.';
  }
  atualizarTempo();
  desenhar();
}

function atualizarTempo() {
  if (!refs) return;
  const g = estado.gravacao;
  refs.tempo.textContent = formatarRelogio(g.ativa ? (Date.now() - g.inicio) / 1000 : 0);
}

function iniciarRelogio() {
  pararRelogio();
  relogio = setInterval(atualizarTempo, 500);
}

function pararRelogio() {
  if (relogio) clearInterval(relogio);
  relogio = null;
}

/** A "fita": um histórico rolante do volume captado, mais recente à direita. */
function desenhar() {
  if (!refs || !refs.canvas.isConnected) return;
  const c = refs.canvas;
  const dpr = window.devicePixelRatio || 1;
  const largura = Math.max(1, Math.round(c.clientWidth * dpr));
  const altura = Math.max(1, Math.round(c.clientHeight * dpr));
  if (c.width !== largura || c.height !== altura) {
    c.width = largura;
    c.height = altura;
  }
  const ctx = c.getContext('2d');
  ctx.clearRect(0, 0, largura, altura);

  const css = getComputedStyle(document.documentElement);
  ctx.fillStyle = (estado.gravacao.ativa ? css.getPropertyValue('--gravando') : css.getPropertyValue('--linha')).trim();

  const passo = 4 * dpr;
  const espessura = 2.5 * dpr;
  const quantas = Math.floor(largura / passo);
  const meio = altura / 2;
  for (let i = 0; i < quantas; i++) {
    const indice = niveis.length - quantas + i;
    const nivel = indice >= 0 ? niveis[indice] : 0;
    const proporcao = Math.min(1, Math.sqrt(nivel) * 1.4);
    const barra = Math.max(2 * dpr, proporcao * altura);
    ctx.fillRect(i * passo, meio - barra / 2, espessura, barra);
  }
}
