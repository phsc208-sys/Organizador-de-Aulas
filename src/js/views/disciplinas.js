import { $, h, limpar, aviso, confirmar } from '../dom.js';
import { api } from '../api.js';
import { estado } from '../state.js';

export function renderLateral(aoSelecionar) {
  const nav = limpar($('#lista-disciplinas'));
  if (!estado.disciplinas.length) {
    nav.append(h('p', { class: 'lista-vazia-lateral' }, 'Nenhuma disciplina ainda.'));
    return;
  }
  for (const d of estado.disciplinas) {
    const gravandoAqui = estado.gravacao.ativa && estado.gravacao.disciplinaId === d.id;
    const sub = [d.professor, d.semestre].filter(Boolean).join(', ');
    nav.append(
      h(
        'button',
        {
          class: 'disciplina-item',
          type: 'button',
          'aria-current': String(d.id === estado.disciplinaId),
          onclick: () => aoSelecionar(d.id),
        },
        h('span', { class: 'nome' }, d.nome),
        sub ? h('span', { class: 'sub' }, sub) : null,
        gravandoAqui ? h('span', { class: 'ao-vivo' }, 'Gravando agora') : null,
      ),
    );
  }
}

export function montarCabecalho(d, { aoEditar, aoExcluir }) {
  const detalhes = [];
  if (d.professor) detalhes.push(h('span', {}, `Prof. ${d.professor}`));
  if (d.semestre) detalhes.push(h('span', {}, `Semestre ${d.semestre}`));
  if (d.carga_horaria) detalhes.push(h('span', {}, `${d.carga_horaria} h`));
  return h(
    'header',
    { class: 'cabecalho' },
    h('div', {}, h('h2', {}, d.nome), detalhes.length ? h('div', { class: 'detalhes' }, detalhes) : null),
    h(
      'div',
      { class: 'acoes-cabecalho' },
      h('button', { class: 'btn btn-discreto btn-pequeno', type: 'button', onclick: aoEditar }, 'Editar'),
      h('button', { class: 'btn btn-discreto btn-pequeno', type: 'button', onclick: aoExcluir }, 'Excluir'),
    ),
  );
}

// ───────── Formulário (criar/editar) ─────────
let editandoId = null;
let aoSalvarCb = null;

export function iniciarDialogDisciplina() {
  const dlg = $('#dlg-disciplina');
  const form = $('#form-disciplina');
  $('[data-fechar]', dlg).addEventListener('click', () => dlg.close());
  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const dados = lerFormulario(form);
    try {
      const salva = editandoId
        ? await api.disciplinas.atualizar(editandoId, dados)
        : await api.disciplinas.criar(dados);
      dlg.close();
      if (aoSalvarCb) await aoSalvarCb(salva);
    } catch (e) {
      aviso(String(e), 'erro');
    }
  });
}

function lerFormulario(form) {
  const v = (nome) => form.elements[nome].value.trim();
  const horas = v('carga_horaria');
  return {
    nome: v('nome'),
    professor: v('professor') || null,
    semestre: v('semestre') || null,
    carga_horaria: horas === '' ? null : parseInt(horas, 10),
  };
}

/** disciplina = null abre o formulário para uma nova disciplina. */
export function abrirFormDisciplina(disciplina, aoSalvar) {
  const dlg = $('#dlg-disciplina');
  const form = $('#form-disciplina');
  editandoId = disciplina ? disciplina.id : null;
  aoSalvarCb = aoSalvar;
  $('#dlg-disciplina-titulo').textContent = disciplina ? 'Editar disciplina' : 'Nova disciplina';
  form.elements.nome.value = disciplina?.nome ?? '';
  form.elements.professor.value = disciplina?.professor ?? '';
  form.elements.semestre.value = disciplina?.semestre ?? '';
  form.elements.carga_horaria.value = disciplina?.carga_horaria ?? '';
  dlg.showModal();
  form.elements.nome.focus();
}

export async function excluirDisciplina(d, depois) {
  const ok = await confirmar({
    titulo: `Excluir "${d.nome}"?`,
    texto:
      'As aulas, transcrições, atas e arquivos de áudio dessa disciplina serão apagados. Não dá para desfazer.',
    acao: 'Excluir disciplina',
    perigo: true,
  });
  if (!ok) return;
  try {
    await api.disciplinas.excluir(d.id);
    aviso('Disciplina excluída.', 'ok');
    await depois();
  } catch (e) {
    aviso(String(e), 'erro');
  }
}
