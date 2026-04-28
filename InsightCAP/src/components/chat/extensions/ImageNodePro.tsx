import { Node, mergeAttributes, nodeInputRule } from '@tiptap/core';
import { ReactNodeViewRenderer } from '@tiptap/react';
import ImageNodeView from './ImageNodeView';

export interface ImageNodeProOptions {
    HTMLAttributes: Record<string, any>;
}

declare module '@tiptap/core' {
    interface Commands<ReturnType> {
        imageNodePro: {
            /**
             * Add an image
             */
            setImage: (options: { src: string; alt?: string; title?: string; width?: string | number; textAlign?: string }) => ReturnType;
        };
    }
}

export const inputRegex = /(?:^|\s)(!\[(.+|:?)]\((\S+)(?:(?:\s+)["'](\S+)["'])?\))$/;

export const ImageNodePro = Node.create<ImageNodeProOptions>({
    name: 'imageNodePro',

    group: 'block',

    draggable: true,
    selectable: true,
    atom: true,

    addAttributes() {
        return {
            src: {
                default: null,
            },
            alt: {
                default: null,
            },
            title: {
                default: null,
            },
            width: {
                default: '100%',
            },
            textAlign: {
                default: 'center',
            },
        };
    },

    parseHTML() {
        return [
            {
                tag: 'div[data-type="image-node-pro"]',
            },
        ];
    },

    renderHTML({ HTMLAttributes }) {
        return ['div', mergeAttributes(this.options.HTMLAttributes, HTMLAttributes, { 'data-type': 'image-node-pro' })];
    },

    addNodeView() {
        return ReactNodeViewRenderer(ImageNodeView);
    },

    addCommands() {
        return {
            setImage:
                options =>
                    ({ commands }) => {
                        return commands.insertContent({
                            type: this.name,
                            attrs: options,
                        });
                    },
        };
    },

    addInputRules() {
        return [
            nodeInputRule({
                find: inputRegex,
                type: this.type,
                getAttributes: match => {
                    const [, , alt, src, title] = match;
                    return { src, alt, title };
                },
            }),
        ];
    },
});
