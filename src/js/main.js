import { $, h, limpar, aviso } from './dom.js';
import { api, aoEvento } from './api.js';
import { estado, disciplinaAtual } from './state.js';
import {
  renderLateral,
  montarCabecalho,
  iniciarDialogDisciplina,
  abrirFormDisciplina,
  excluirDisciplina,
} from './views/disciplinas.js';
import {
  montarGravacao,
  registrarEventosGravacao,
  atualizarAvisoAmbiente,
} from './views/gravacao.js';
import {
  montarHistorico,
  carregarRegistros,
  recarregarSelecionado,
  renderHistorico,
} from './views/historico.js';
import { iniciarConfig } from './views/config.js';

const CHAVE_ULTIMA = 'ultima-disciplina';

function lerUltima() {
  try {
    return parseInt(localStorage.getItem(CHAVE_ULTIMA), 10);
  } catch (_) {
    return NaN;
  }
}

function guardarUltima(id) {
  try {
    localStorage.setItem(CHAVE_ULTIMA, String(id));
  } catch (_) { /* sem armazenamento: tudo bem */ }
}

async function recarregarDisciplinas() {
  estado.disciplinas = await api.disciplinas.listar();
}

async function selecionarDisciplina(id) {
  estado.disciplinaId = id;
  estado.registroId = null;
  estado.registro = null;
  guardarUltima(id);
  try {
    await carregarRegistros();
  } catch (e) {
    aviso(String(e), 'erro');
  }
  renderLateral(selecionarDisciplina);
  renderPrincipal();
}

function vazio() {
  const semNenhuma = estado.disciplinas.length === 0;
  return h(
    'section',
    { class: 'vazio' },
    h('h2', {}, semNenhuma ? 'Comece criando uma disciplina' : 'Escolha uma disciplina'),
    h(
      'p',
      {},
      semNenhuma
        ? 'Cada aula gravada fica ligada a uma disciplina, com a transcrição e a ata.'
        : 'Selecione uma disciplina na lista para gravar uma aula ou rever as atas.',
    ),
    semNenhuma
      ? h(
          'button',
          { class: 'btn btn-primario', type: 'button', onclick: () => $('#btn-nova-disciplina').click() },
          'Nova disciplina',
        )
      : null,
  );
}

function renderPrincipal() {
  const raiz = limpar($('#principal'));
  const d = disciplinaAtual();
  if (!d) {
    raiz.append(vazio());
    return;
  }

  raiz.append(
    montarCabecalho(d, {
      aoEditar: () =>
        abrirFormDisciplina(d, async () => {
          await recarregarDisciplinas();
          renderLateral(selecionarDisciplina);
          renderPrincipal();
        }),
      aoExcluir: () =>
        excluirDisciplina(d, async () => {
          await recarregarDisciplinas();
          const proxima = estado.disciplinas[0]?.id ?? null;
          if (proxima) await selecionarDisciplina(proxima);
          else {
            estado.disciplinaId = null;
            estado.registros = [];
            renderLateral(selecionarDisciplina);
            renderPrincipal();
          }
        }),
    }),
  );

  const zonaGravacao = h('div');
  const zonaHistorico = h('div');
  raiz.append(zonaGravacao, zonaHistorico);

  montarGravacao(zonaGravacao, {
    aoMudar: () => renderLateral(selecionarDisciplina),
    aoFinalizar: async (registroId, disciplinaGravada) => {
      if (disciplinaGravada !== estado.disciplinaId) await selecionarDisciplina(disciplinaGravada);
      await carregarRegistros();
      estado.registroId = registroId;
      estado.aba = null;
      await recarregarSelecionado();
      renderHistorico();
    },
  });
  montarHistorico(zonaHistorico);
}

async function aoEtapaDoPipeline(p) {
  if (estado.registros.some((r) => r.id === p.registro_id)) {
    try {
      await carregarRegistros();
      if (estado.registroId === p.registro_id) await recarregarSelecionado();
      renderHistorico();
    } catch (e) {
      aviso(String(e), 'erro');
    }
  }
  if (p.etapa === 'concluido') aviso('Ata pronta.', 'ok');
  if (p.etapa === 'erro') aviso(`Falha ao processar a aula: ${p.mensagem}`, 'erro');
}

async function iniciar() {
  iniciarDialogDisciplina();
  iniciarConfig({
    aoSalvar: async () => {
      estado.ambiente = await api.config.ambiente();
      atualizarAvisoAmbiente();
    },
  });

  $('#btn-nova-disciplina').addEventListener('click', () =>
    abrirFormDisciplina(null, async (nova) => {
      await recarregarDisciplinas();
      await selecionarDisciplina(nova.id);
    }),
  );

  await registrarEventosGravacao();
  await aoEvento('pipeline:etapa', aoEtapaDoPipeline);

  try {
    const [ambiente, rec] = await Promise.all([api.config.ambiente(), api.gravacao.estado()]);
    estado.ambiente = ambiente;
    if (rec.gravando) {
      estado.gravacao = {
        ativa: true,
        disciplinaId: rec.disciplina_id,
        inicio: Date.now() - rec.segundos * 1000,
      };
    }
    await recarregarDisciplinas();
  } catch (e) {
    aviso(String(e), 'erro');
  }

  const ultima = lerUltima();
  const inicial = estado.gravacao.ativa
    ? estado.gravacao.disciplinaId
    : estado.disciplinas.some((d) => d.id === ultima)
      ? ultima
      : (estado.disciplinas[0]?.id ?? null);

  if (inicial) await selecionarDisciplina(inicial);
  else {
    renderLateral(selecionarDisciplina);
    renderPrincipal();
  }
}

iniciar();
