// Desfoque (vidro fosco) atrás das janelas do Chube (com.pedro.Chube): a mesma camada da dock,
// do painel e dos menus (desfoque.js), dentro da janela, por baixo do conteúdo.
//
// Quem decide é o Chube. Ele grava em ~/.config/chube/desfoque "on <raio> <brilho> <cantos>"
// (raio e cantos em px lógicos, brilho em %, como nos ajustes da dock) quando o tema dele é
// translúcido e o desfoque está ligado no menu [◐], e "off" quando não. Esta extensão só
// acompanha o arquivo.
//
// A camada fica como primeiro filho do MetaWindowActor (como na temas-janelas@pedro, que faz o
// mesmo para os apps libadwaita): anda, some e anima junto com a janela, e fica sempre logo abaixo
// dela no empilhamento. Ela copia só as janelas de baixo.
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

import {BaseCamada, CamadaDesfocada, EfeitoCantos} from './desfoque.js';

const APP_ID = 'com.pedro.Chube';
const ARQUIVO = 'desfoque';

// "on 30 85 12" → {raio: 30, brilho: 0.85, cantos: 12}; outra coisa (ou sem arquivo) → null.
function lerPedido(arquivo) {
    try {
        const [, bytes] = arquivo.load_contents(null);
        const [estado, raio, brilho, cantos] = new TextDecoder().decode(bytes).trim().split(/\s+/);
        if (estado !== 'on')
            return null;
        return {raio: Number(raio) || 30, brilho: (Number(brilho) || 85) / 100, cantos: Number(cantos) || 0};
    } catch {
        return null;
    }
}

export default class DesfoqueChube extends Extension {
    enable() {
        // Janelas do Chube (com a camada) e todas as janelas já vistas (com os sinais ligados).
        this._janelas = new Map();
        this._vistas = new Set();
        const pasta = Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_config_dir(), 'chube']));
        this._arquivo = pasta.get_child(ARQUIVO);
        this._pedido = lerPedido(this._arquivo);

        // A pasta, não o arquivo: o Chube grava com rename, que troca o arquivo de lugar.
        try {
            pasta.make_directory_with_parents(null);
        } catch {
            // Já existe.
        }
        this._monitor = pasta.monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        this._monitor.connect('changed', (_monitor, arquivo, outro) => {
            if ([arquivo, outro].some(f => f?.get_basename() === ARQUIVO)) {
                this._pedido = lerPedido(this._arquivo);
                this._janelas.forEach(info => this._aplicar(info));
            }
        });

        global.display.connectObject('window-created', (_display, janela) => this._acompanhar(janela), this);
        // Janela criada antes de ter ator: entra quando aparecer.
        global.window_manager.connectObject('map', (_wm, ator) => this._acompanhar(ator.meta_window), this);
        St.ThemeContext.get_for_stage(global.stage).connectObject('notify::scale-factor',
            () => this._janelas.forEach(info => this._aplicar(info)), this);
        for (const ator of global.get_window_actors())
            this._acompanhar(ator.meta_window);
    }

    disable() {
        this._monitor.cancel();
        this._monitor = null;
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        for (const janela of [...this._janelas.keys()])
            this._largar(janela);
        for (const janela of this._vistas)
            janela.disconnectObject(this);
        this._janelas = null;
        this._vistas = null;
    }

    // No Wayland, o app_id só chega depois da janela criada, e o ator pode faltar no
    // window-created: confere de novo quando o app_id mudar e quando a janela aparecer.
    _acompanhar(janela) {
        if (!this._vistas.has(janela)) {
            this._vistas.add(janela);
            janela.connectObject(
                'notify::wm-class', () => this._conferir(janela),
                'unmanaged', () => this._vistas.delete(janela), this);
        }
        this._conferir(janela);
    }

    _conferir(janela) {
        if (janela.get_wm_class() !== APP_ID || this._janelas.has(janela))
            return;
        const ator = janela.get_compositor_private();
        if (!ator)
            return;
        const info = {janela, ator, base: null, camada: null, desfocado: null, cantos: null};
        this._janelas.set(janela, info);
        // Os sinais da janela vão com o próprio info como dono: os de cima (de this) continuam.
        janela.connectObject(
            'size-changed', () => this._ajustar(info),
            'position-changed', () => this._ajustar(info),
            'notify::maximized-horizontally', () => this._ajustar(info),
            'notify::maximized-vertically', () => this._ajustar(info),
            'notify::fullscreen', () => this._ajustar(info),
            'unmanaging', () => this._largar(janela), info);
        // A camada é filha do ator e some com ele.
        ator.connectObject('destroy', () => {
            info.base = null;
            this._largar(janela);
        }, info);
        this._aplicar(info);
    }

    _largar(janela) {
        const info = this._janelas.get(janela);
        if (!info)
            return;
        janela.disconnectObject(info);
        info.ator.disconnectObject(info);
        info.base?.destroy();
        this._janelas.delete(janela);
    }

    _aplicar(info) {
        if (!this._pedido) {
            info.base?.destroy();
            info.base = null;
            return;
        }
        if (!info.base)
            this._desfocar(info);
        const escala = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        info.desfocado.ajustar(this._pedido.raio * escala, this._pedido.brilho);
        this._ajustar(info);
    }

    // Como na temas-janelas@pedro: o ator da janela escala os filhos pela escala do monitor (as
    // coordenadas deles são lógicas); a base desfaz essa escala, e dentro dela tudo volta a ser em
    // pixels da tela, como a cópia espera. A camada recorta a cópia com os cantos da janela.
    _desfocar(info) {
        info.desfocado = new CamadaDesfocada({x_expand: true, y_expand: true}, info.ator);
        info.camada = new Clutter.Actor({layout_manager: new Clutter.BinLayout(), clip_to_allocation: true});
        info.camada.add_child(info.desfocado);
        info.cantos = new EfeitoCantos();
        info.camada.add_effect(info.cantos);
        info.base = new BaseCamada({name: 'desfoque-chube'});
        info.base.add_child(info.camada);
        info.ator.insert_child_at_index(info.base, 0);
        // A janela anda: a cópia precisa ser reposta (sem esperar, para não ficar um quadro atrás).
        const repor = () => info.desfocado.queue_relayout();
        info.ator.connectObject('notify::x', repor, 'notify::y', repor,
            'notify::translation-x', repor, 'notify::translation-y', repor, info.base);
    }

    // Lugar (o retângulo da janela em relação ao ator, que cobre também a borda de redimensionar)
    // e cantos: os do libcosmic, retos com a janela maximizada ou em tela cheia.
    _ajustar({janela, ator, base, camada, desfocado, cantos}) {
        if (!base || !this._pedido)
            return;
        const escala = ator.get_child_transform()?.get_value(0, 0) || 1;
        if (base.scale_x !== 1 / escala)
            base.set_scale(1 / escala, 1 / escala);
        const quadro = janela.get_frame_rect();
        const buffer = janela.get_buffer_rect();
        camada.set_position(quadro.x - buffer.x, quadro.y - buffer.y);
        camada.set_size(quadro.width, quadro.height);
        const reta = janela.get_maximized() !== 0 || janela.is_fullscreen();
        const raio = reta ? 0 : this._pedido.cantos * global.display.get_monitor_scale(janela.get_monitor());
        cantos.definir({largura: quadro.width, altura: quadro.height,
            raio_se: raio, raio_sd: raio, raio_id: raio, raio_ie: raio});
        desfocado.queue_relayout();
    }
}
