// Única camada que fala com o Rust. Os argumentos de nível superior vão em camelCase
// (o Tauri converte para snake_case); os campos dentro de objetos seguem o snake_case do Rust.
const tauri = window.__TAURI__;
if (!tauri) {
  throw new Error('API do Tauri indisponível. Abra o app com "npm run dev", não direto no navegador.');
}
const { invoke } = tauri.core;
const { listen } = tauri.event;

export const api = {
  disciplinas: {
    listar: () => invoke('listar_disciplinas'),
    criar: (dados) => invoke('criar_disciplina', { dados }),
    atualizar: (id, dados) => invoke('atualizar_disciplina', { id, dados }),
    excluir: (id) => invoke('excluir_disciplina', { id }),
  },
  registros: {
    listar: (disciplinaId) => invoke('listar_registros', { disciplinaId }),
    obter: (id) => invoke('obter_registro', { id }),
    excluir: (id) => invoke('excluir_registro', { id }),
    reprocessar: (id, desde) => invoke('reprocessar_registro', { id, desde }),
  },
  gravacao: {
    estado: () => invoke('estado_gravacao'),
    iniciar: (disciplinaId) => invoke('iniciar_gravacao', { disciplinaId }),
    parar: () => invoke('parar_gravacao'),
    dispositivos: () => invoke('listar_dispositivos_entrada'),
  },
  config: {
    obter: () => invoke('obter_configuracoes'),
    salvar: (dados) => invoke('salvar_configuracoes', { dados }),
    ambiente: () => invoke('verificar_ambiente'),
  },
};

/** Escuta um evento do backend. Devolve uma Promise com a função de cancelamento. */
export const aoEvento = (nome, funcao) => listen(nome, (evento) => funcao(evento.payload));
