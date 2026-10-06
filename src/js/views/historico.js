import {
  h,
  limpar,
  aviso,
  confirmar,
  formatarDataHora,
  formatarDataLonga,
  formatarDuracao,
} from '../dom.js';
import { api } from '../api.js';
import { estado, nomeDaDisciplina } from '../state.js';

const ROTULOS = {
  gravado: 'Na fila para transcrever',
  transcrevendo: 'Transcrevendo',
  transcrito: 'Transcrita, falta a ata',
  resumindo: 'Gerando a ata',
  concluido: 'Ata pronta',
  erro: 'Falhou',
};

const EM_ANDAMENTO = new Set(['gravado', 'transcrevendo', 'resumindo']);

let refs = null;

export function montarHistorico(destino) {
  refs = {
    lista: h('div', { class: 'aulas' }),
    leitor: h('article', { class: 'leitor', 'aria-live': 'polite' }),
  };
  destino.append(
    h(
      'section',
      { class: 'historico' },
      h('div', {}, h('h3', { class: 'titulo-secao' }, 'Aulas gravadas'), refs.lista),
      refs.leitor,
    ),
  );
  renderHistorico();
}

export async function carregarRegistros() {
  estado.registros = estado.disciplinaId ? await api.registros.listar(estado.disciplinaId) : [];
  if (estado.registroId && !estado.registros.some((r) => r.id === estado.registroId)) {
    estado.registroId = null;
    estado.registro = null;
  }
}

export async function recarregarSelecionado() {
  if (!estado.registroId) {
    estado.registro = null;
    return;
  }
  try {
    estado.registro = await api.registros.obter(estado.registroId);
  } catch (e) {
    estado.registro = null;
    estado.registroId = null;
    aviso(String(e), 'erro');
  }
}

export async function selecionarRegistro(id) {
  estado.registroId = id;
  estado.aba = null; // deixa o leitor escolher a aba mais útil
  await recarregarSelecionado();
  renderHistorico();
}

export function renderHistorico() {
  if (!refs || !refs.lista.isConnected) return;
  renderLista();
  renderLeitor();
}

function renderLista() {
  limpar(refs.lista);
  if (!estado.registros.length) {
    refs.lista.append(
      h('p', { class: 'lista-vazia' }, 'Nenhuma aula gravada nesta disciplina ainda.'),
    );
    return;
  }
  for (const r of estado.registros) {
    refs.lista.append(
      h(
        'button',
        {
          class: 'aula',
          type: 'button',
          'aria-current': String(r.id === estado.registroId),
          onclick: () => selecionarRegistro(r.id),
        },
        h('span', { class: 'titulo' }, r.titulo || 'Aula gravada'),
        h(
          'span',
          { class: 'linha' },
          h('span', {}, formatarDataHora(r.data_hora)),
          h('span', {}, formatarDuracao(r.duracao_seg)),
        ),
        h(
          'span',
          { class: 'linha' },
          h('span', { class: 'estado', dataset: { status: r.status } }, ROTULOS[r.status] || r.status),
        ),
      ),
    );
  }
}

function lerAta(texto) {
  if (!texto) return null;
  try {
    return JSON.parse(texto);
  } catch (_) {
    return null;
  }
}

function renderLeitor() {
  const alvo = limpar(refs.leitor);
  const r = estado.registro;
  if (!r) {
    alvo.append(
      h(
        'p',
        { class: 'leitor-vazio' },
        estado.registros.length
          ? 'Escolha uma aula na lista para ler a ata e a transcrição.'
          : 'Quando você gravar uma aula, a ata aparece aqui.',
      ),
    );
    return;
  }

  const ata = lerAta(r.ata_gemini);
  const temTranscricao = Boolean(r.transcricao_bruta && r.transcricao_bruta.trim());
  const andamento = EM_ANDAMENTO.has(r.status);

  alvo.append(
    h(
      'div',
      { class: 'leitor-topo' },
      h(
        'div',
        {},
        h('h3', {}, ata?.titulo || 'Aula gravada'),
        h(
          'div',
          { class: 'meta' },
          `${formatarDataLonga(r.data_hora)}, ${formatarDuracao(r.duracao_seg)}`,
        ),
      ),
      h(
        'div',
        { class: 'leitor-acoes' },
        ata
          ? h('button', { class: 'btn btn-pequeno', type: 'button', onclick: () => copiarAta(ata, r) }, 'Copiar ata')
          : null,
        !andamento
          ? h(
              'button',
              { class: 'btn btn-discreto btn-pequeno', type: 'button', onclick: () => excluirAula(r) },
              'Excluir aula',
            )
          : null,
      ),
    ),
  );

  if (andamento) {
    alvo.append(
      h(
        'div',
        { class: 'caixa-estado' },
        h('strong', {}, ROTULOS[r.status]),
        h(
          'p',
          {},
          'Aulas longas levam vários minutos. O processamento segue em segundo plano e você pode gravar outra aula, mas não feche o app até terminar.',
        ),
      ),
    );
  }

  if (r.status === 'erro') {
    const botoes = [];
    if (temTranscricao) {
      botoes.push(
        h(
          'button',
          { class: 'btn btn-primario btn-pequeno', type: 'button', onclick: () => reprocessar(r.id, 'ata') },
          'Gerar a ata de novo',
        ),
      );
    }
    botoes.push(
      h(
        'button',
        {
          class: temTranscricao ? 'btn btn-pequeno' : 'btn btn-primario btn-pequeno',
          type: 'button',
          onclick: () => reprocessar(r.id, 'transcricao'),
        },
        'Transcrever de novo',
      ),
    );
    alvo.append(
      h(
        'div',
        { class: 'caixa-estado', dataset: { tipo: 'erro' } },
        h('strong', {}, 'O processamento falhou'),
        h('p', {}, r.erro || 'Erro desconhecido.'),
        temTranscricao
          ? h('p', {}, 'A transcrição está salva, então dá para tentar só a ata outra vez.')
          : null,
        h('div', { class: 'botoes' }, botoes),
      ),
    );
  }

  const abas = [];
  if (ata) abas.push(['ata', 'Ata']);
  if (temTranscricao) abas.push(['transcricao', 'Transcrição']);
  if (!abas.length) return;

  const abaAtual = abas.some(([id]) => id === estado.aba) ? estado.aba : abas[0][0];
  alvo.append(
    h(
      'div',
      { class: 'abas', role: 'tablist' },
      abas.map(([id, rotulo]) =>
        h(
          'button',
          {
            class: 'aba',
            type: 'button',
            role: 'tab',
            'aria-selected': String(id === abaAtual),
            onclick: () => {
              estado.aba = id;
              renderLeitor();
            },
          },
          rotulo,
        ),
      ),
    ),
  );

  alvo.append(
    abaAtual === 'ata'
      ? renderAta(ata)
      : h('div', { class: 'transcricao' }, r.transcricao_bruta),
  );
}

function paragrafos(texto) {
  return String(texto || '')
    .split(/\n{2,}/)
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => h('p', {}, p));
}

function renderAta(ata) {
  const partes = [h('h4', {}, 'Resumo'), ...paragrafos(ata.resumo)];

  if (ata.topicos_principais?.length) {
    partes.push(
      h('h4', {}, 'Tópicos principais'),
      ata.topicos_principais.map((t) =>
        h('div', { class: 'topico' }, h('strong', {}, t.titulo), h('p', {}, t.explicacao)),
      ),
    );
  }
  if (ata.datas_importantes?.length) {
    partes.push(
      h('h4', {}, 'Datas importantes'),
      h(
        'dl',
        {},
        ata.datas_importantes.flatMap((d) => [h('dt', {}, d.data), h('dd', {}, d.descricao)]),
      ),
    );
  }
  if (ata.tarefas_e_avisos?.length) {
    partes.push(
      h('h4', {}, 'Tarefas e avisos'),
      h('ul', {}, ata.tarefas_e_avisos.map((t) => h('li', {}, t))),
    );
  }
  return h('div', { class: 'ata' }, partes);
}

function ataParaMarkdown(ata, registro) {
  const linhas = [`# ${ata.titulo}`, ''];
  const disciplina = nomeDaDisciplina(registro.disciplina_id);
  if (disciplina) linhas.push(`**Disciplina:** ${disciplina}`);
  linhas.push(`**Data:** ${formatarDataHora(registro.data_hora)}`, '', '## Resumo', '', ata.resumo, '');
  if (ata.topicos_principais?.length) {
    linhas.push('## Tópicos principais', '');
    for (const t of ata.topicos_principais) linhas.push(`### ${t.titulo}`, '', t.explicacao, '');
  }
  if (ata.datas_importantes?.length) {
    linhas.push('## Datas importantes', '');
    for (const d of ata.datas_importantes) linhas.push(`- **${d.data}:** ${d.descricao}`);
    linhas.push('');
  }
  if (ata.tarefas_e_avisos?.length) {
    linhas.push('## Tarefas e avisos', '');
    for (const t of ata.tarefas_e_avisos) linhas.push(`- ${t}`);
    linhas.push('');
  }
  return linhas.join('\n');
}

async function copiarAta(ata, registro) {
  try {
    await navigator.clipboard.writeText(ataParaMarkdown(ata, registro));
    aviso('Ata copiada em Markdown.', 'ok');
  } catch (_) {
    aviso('Não foi possível copiar. Selecione o texto da ata e copie manualmente.', 'erro');
  }
}

async function reprocessar(id, desde) {
  try {
    await api.registros.reprocessar(id, desde);
    await carregarRegistros();
    await recarregarSelecionado();
    renderHistorico();
  } catch (e) {
    aviso(String(e), 'erro');
  }
}

async function excluirAula(r) {
  const ok = await confirmar({
    titulo: 'Excluir esta aula?',
    texto: 'A ata, a transcrição e o arquivo de áudio serão apagados. Não dá para desfazer.',
    acao: 'Excluir aula',
    perigo: true,
  });
  if (!ok) return;
  try {
    await api.registros.excluir(r.id);
    estado.registroId = null;
    estado.registro = null;
    await carregarRegistros();
    renderHistorico();
    aviso('Aula excluída.', 'ok');
  } catch (e) {
    aviso(String(e), 'erro');
  }
}
