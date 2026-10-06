-- Estado de cada estágio do pipeline, para permitir reprocessamento.
-- status: gravado | transcrevendo | transcrito | resumindo | concluido | erro
ALTER TABLE registros_aula ADD COLUMN status TEXT NOT NULL DEFAULT 'gravado';
ALTER TABLE registros_aula ADD COLUMN erro TEXT;
ALTER TABLE registros_aula ADD COLUMN duracao_seg INTEGER;
