export const estado = {
  disciplinas: [],
  disciplinaId: null,
  registros: [],
  registroId: null,
  registro: null,
  aba: 'ata',
  gravacao: { ativa: false, disciplinaId: null, inicio: null },
  ambiente: null,
};

export const disciplinaAtual = () =>
  estado.disciplinas.find((d) => d.id === estado.disciplinaId) || null;

export const nomeDaDisciplina = (id) =>
  estado.disciplinas.find((d) => d.id === id)?.nome ?? '';
