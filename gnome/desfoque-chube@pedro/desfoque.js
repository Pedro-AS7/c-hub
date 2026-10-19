// Desfoque (vidro fosco) para a dock, o painel e os menus. Arquivo igual em temas-dock@pedro,
// temas-painel@pedro e temas-menus@pedro: ao mudar um, copie para os outros.
//
// O desfoque do que está atrás de um ator (Shell.BlurEffect no modo BACKGROUND) não desfocou nada
// no teste de 2026-10-15 e, na dock, sairia retangular, com os cantos quadrados para fora do
// fundo arredondado. Então a camada desenha uma cópia do que está atrás dela, posta exatamente
// sobre o mesmo lugar da tela, e desfoca essa cópia como ator (modo ACTOR), que pode ser
// recortado depois (EfeitoCantos).
//
// A cópia é feita de um Clutter.Clone do papel de parede e um de cada janela, em ordem de
// empilhamento. Um Clone do global.window_group inteiro não serve: ele tem tamanho 0, e o Clone
// se escala pela razão entre os tamanhos (0/0), não desenhando nada.
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// Põe o palco (onde ficam as cópias, nas mesmas coordenadas do window_group) de modo que cada
// ponto dele caia sobre o mesmo ponto da tela, onde quer que a camada esteja. Roda depois que
// os pais já ganharam a posição nova; quem move a camada sem mudar o tamanho dela precisa
// chamar queue_relayout().
const LayoutCopia = GObject.registerClass(
class LayoutCopia extends Clutter.LayoutManager {
    vfunc_get_preferred_width() {
        return [0, 0];
    }

    vfunc_get_preferred_height() {
        return [0, 0];
    }

    vfunc_allocate(camada) {
        const [x, y] = camada.get_transformed_position();
        const [ox, oy] = global.window_group.get_transformed_position();
        const [largura, altura] = global.stage.get_size();
        for (const filho of camada.get_children())
            filho.allocate(Clutter.ActorBox.new(ox - x, oy - y, ox - x + largura, oy - y + altura));
    }
});

function copiar(fonte) {
    const copia = new Clutter.Clone({source: fonte});
    // Mesmo lugar e tamanho da fonte (tamanho igual = escala 1).
    copia.add_constraint(new Clutter.BindConstraint({source: fonte, coordinate: Clutter.BindCoordinate.ALL}));
    return copia;
}

// Quantas camadas estão desenhando a cópia delas agora (uma pode estar dentro da outra). Fica no
// globalThis, e não numa variável deste arquivo, porque cada extensão carrega a própria cópia
// dele: a camada da janela do Chube precisa calar o vidro de uma janela da temas@pedro.
const COPIANDO = 'desfoque@pedro:copiando';

function copiando() {
    return globalThis[COPIANDO] ?? 0;
}

// Onde o vidro de uma janela entra no ator dela (por baixo do conteúdo). Dentro da cópia que
// outra camada faz dessa janela, o vidro não se desenha: desfocar de novo o que já é desfocado
// quase não muda nada, e desenhá-lo lá estragava o resultado que os efeitos dele guardam para
// reusar. Com as janelas em monitores diferentes, a cópia é desenhada na vez do outro monitor,
// e a janela de baixo ficava transparente, sem o vidro, até algo atrás dela mudar (visto em
// 2026-10-16 ao passar o foco para outra janela com vidro).
export const BaseCamada = GObject.registerClass(
class BaseCamada extends Clutter.Actor {
    vfunc_paint(contexto) {
        if (!copiando())
            super.vfunc_paint(contexto);
    }
});

// Ator que mostra, desfocado, o que está atrás dele: papel de parede e janelas. Com abaixoDe (o
// ator de uma janela), só as janelas abaixo dela entram na cópia (a camada fica dentro dela, e
// copiá-la a desenharia dentro de si mesma).
//
// Na visão geral, o grupo das janelas fica escondido e o que aparece atrás da dock, dos menus e
// das notificações é a própria visão geral: a cópia passa a ser dela (o overviewGroup, que cobre
// a tela inteira em 0, 0, como o grupo das janelas). Antes, essas camadas saíam na visão geral
// (a cópia mostraria as janelas onde estavam), e tudo ficava transparente. A camada de uma
// janela (abaixoDe) continua com a cópia de sempre: ela é desenhada dentro da miniatura da
// janela, que fica dentro da visão geral, e copiá-la a desenharia dentro de si mesma.
export const CamadaDesfocada = GObject.registerClass(
class CamadaDesfocada extends Clutter.Actor {
    constructor(params = {}, abaixoDe = null) {
        super({...params, layout_manager: new LayoutCopia(), clip_to_allocation: true});
        this._abaixoDe = abaixoDe;
        this.desfoque = new Shell.BlurEffect({mode: Shell.BlurMode.ACTOR});
        this.add_effect(this.desfoque);

        this._palco = new Clutter.Actor();
        this.add_child(this._palco);
        // Papel de parede e janelas juntos, para trocá-los de uma vez pela visão geral.
        this._mesa = new Clutter.Actor();
        this._palco.add_child(this._mesa);
        this._mesa.add_child(copiar(Main.layoutManager._backgroundGroup));
        this._janelas = [];

        // "restacked" vem com janela aberta, fechada, levantada ou trocada de workspace.
        global.display.connectObject('restacked', () => this._copiarJanelas(), this);
        this._copiarJanelas();

        const visaoGeral = Main.layoutManager.overviewGroup;
        if (!abaixoDe) {
            this._visaoGeral = copiar(visaoGeral);
            this._palco.add_child(this._visaoGeral);
            // Ele aparece e some junto com o grupo das janelas (layout.js, _updateVisibility).
            visaoGeral.connectObject('notify::visible', () => this._trocarCopia(), this);
            this._trocarCopia();
        }
        this.connect('destroy', () => {
            global.display.disconnectObject(this);
            visaoGeral.disconnectObject(this);
        });
    }

    _trocarCopia() {
        const naVisaoGeral = Main.layoutManager.overviewGroup.visible;
        this._mesa.visible = !naVisaoGeral;
        this._visaoGeral.visible = naVisaoGeral;
    }

    // Uma cópia por janela, de baixo para cima. Janela escondida (minimizada, de outro
    // workspace) tem a cópia escondida também: um Clone desenha a fonte mesmo escondida.
    _copiarJanelas() {
        this._janelas.forEach(copia => copia.destroy());
        let janelas = global.get_window_actors();
        if (this._abaixoDe) {
            const posicao = janelas.indexOf(this._abaixoDe);
            janelas = posicao >= 0 ? janelas.slice(0, posicao) : [];
        }
        this._janelas = janelas.map(janela => {
            const copia = copiar(janela);
            janela.bind_property('visible', copia, 'visible', GObject.BindingFlags.SYNC_CREATE);
            this._mesa.add_child(copia);
            return copia;
        });
    }

    vfunc_paint(contexto) {
        // Uma camada dentro do que ela mesma copia (um menu aberto dentro da visão geral) se
        // desenharia dentro de si sem fim.
        if (this._pintando)
            return;
        this._pintando = true;
        globalThis[COPIANDO] = copiando() + 1;
        try {
            super.vfunc_paint(contexto);
        } finally {
            globalThis[COPIANDO] = copiando() - 1;
            this._pintando = false;
        }
    }

    ajustar(raio, brilho) {
        this.desfoque.radius = raio;
        this.desfoque.brightness = brilho;
    }
});

// Recorta o ator com cantos arredondados (um raio por canto, nas unidades do palco).
const SHADER_CANTOS = `
uniform sampler2D tex;
uniform float largura;
uniform float altura;
uniform float raio_se;
uniform float raio_sd;
uniform float raio_id;
uniform float raio_ie;

void main() {
    vec2 uv = cogl_tex_coord_in[0].xy;
    vec2 meio = vec2(largura, altura) * 0.5;
    vec2 p = uv * vec2(largura, altura) - meio;
    float r = p.y < 0.0 ? (p.x < 0.0 ? raio_se : raio_sd) : (p.x < 0.0 ? raio_ie : raio_id);
    r = min(r, min(meio.x, meio.y));
    vec2 q = abs(p) - meio + vec2(r);
    float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
    cogl_color_out = texture2D(tex, uv) * clamp(0.5 - d, 0.0, 1.0);
}`;

export const EfeitoCantos = GObject.registerClass(
class EfeitoCantos extends Clutter.ShaderEffect {
    constructor() {
        super();
        this.set_shader_source(SHADER_CANTOS);
    }

    definir(uniformes) {
        for (const [nome, valor] of Object.entries(uniformes)) {
            const v = new GObject.Value();
            v.init(GObject.TYPE_FLOAT);
            v.set_float(valor);
            this.set_uniform_value(nome, v);
        }
        this.queue_repaint();
    }
});
