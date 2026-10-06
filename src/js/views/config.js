import { $, h, limpar, aviso } from '../dom.js';
import { api } from '../api.js';

export function iniciarConfig({ aoSalvar }) {
  const dlg = $('#dlg-config');
  const form = $('#form-config');

  $('#btn-config').addEventListener('click', () => abrir(dlg, form));
  $('[data-fechar]', dlg).addEventListener('click', () => dlg.close());

  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const v = (nome) => form.elements[nome].value.trim();
    try {
      await api.config.salvar({
        gemini_api_key: v('gemini_api_key') || null,
        gemini_model: v('gemini_model'),
        whisper_model: v('whisper_model'),
        whisper_language: v('whisper_language'),
        whisper_threads: parseInt(v('whisper_threads'), 10),
        input_device: v('input_device'),
        pulse_source: v('pulse_source'),
      });
      dlg.close();
      aviso('Configurações salvas.', 'ok');
      await aoSalvar();
    } catch (e) {
      aviso(String(e), 'erro');
    }
  });
}

async function abrir(dlg, form) {
  try {
    const [cfg, ambiente, dispositivos] = await Promise.all([
      api.config.obter(),
      api.config.ambiente(),
      api.gravacao.dispositivos().catch(() => []),
    ]);

    for (const campo of [
      'gemini_model',
      'whisper_model',
      'whisper_language',
      'whisper_threads',
      'input_device',
      'pulse_source',
    ]) {
      form.elements[campo].value = cfg[campo];
    }
    form.elements.gemini_api_key.value = '';
    form.elements.gemini_api_key.placeholder = cfg.gemini_api_key_definida
      ? 'Já definida. Deixe vazio para manter.'
      : 'Cole a chave aqui';
    $('#ajuda-chave').textContent = cfg.gemini_api_key_definida
      ? 'A chave fica só no seu computador e nunca vai para a interface.'
      : 'Também vale a variável de ambiente GEMINI_API_KEY.';

    const lista = limpar($('#lista-dispositivos'));
    for (const nome of dispositivos) lista.append(h('option', { value: nome }));

    $('#config-ambiente').textContent = ambiente.modelo_whisper_ok
      ? `Modelo do Whisper encontrado em ${ambiente.caminho_modelo}.`
      : `Modelo do Whisper não encontrado em ${ambiente.caminho_modelo}. Rode scripts/download-model.sh.`;

    dlg.showModal();
  } catch (e) {
    aviso(String(e), 'erro');
  }
}
