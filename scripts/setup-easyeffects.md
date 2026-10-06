# Filtro de ruído: PipeWire + EasyEffects (RNNoise)

O app grava de uma fonte virtual que já passou pelo filtro neural, então digitação e
ventoinha são removidas antes de o áudio chegar ao arquivo.

## 1. Instalar e ligar o filtro

```bash
sudo apt install easyeffects          # ou: flatpak install flathub com.github.wwmm.easyeffects
```

1. Abra o EasyEffects e vá na aba **Microfone** (entrada).
2. Adicione o plugin **Redução de ruído** (usa o RNNoise) e deixe-o ligado.
3. Nas preferências, ative **Rodar em segundo plano / iniciar com o sistema**, para o filtro
   existir mesmo com a janela fechada.

## 2. Descobrir o nome da fonte virtual

```bash
pactl list short sources
```

Procure algo como `easyeffects_source`. Esse é o valor de **Fonte do PipeWire** nas
Configurações do app.

## 3. Configurar o app

Em **Configurações**:

- **Fonte do PipeWire**: `easyeffects_source`
- **Dispositivo de entrada**: `pulse` (ou `pipewire`). O cpal usa o backend ALSA, que não
  lista as fontes do PipeWire pelo nome; ele escolhe a ponte "pulse" e o app aponta essa
  ponte para a fonte do EasyEffects.

Se o dispositivo configurado não existir, o app recusa gravar em vez de cair no microfone
cru. A lista de dispositivos que o app enxerga aparece no próprio menu de Configurações.

## 4. Testar fora do app

```bash
PULSE_SOURCE=easyeffects_source arecord -D pulse -f S16_LE -r 16000 -c 1 -d 5 teste.wav
aplay teste.wav
```

Digite no teclado durante a gravação: o barulho não deve aparecer no arquivo.
