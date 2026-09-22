'use client';

import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef
} from 'react';
import type {
  RichTextDocument,
  RichTextProfileId
} from './generated/contracts';
import type { RichTextMessages } from './messages';
import type { RichTextDirection } from './authoring';
import {
  connectRichTextFrame,
  type RichTextFrameController
} from './frame/controller';

export interface RichTextEditorHandle {
  focus(): void;
  requestDocument(): void;
}

export interface RichTextEditorProps {
  frameUrl: string;
  label: string;
  profile: RichTextProfileId;
  value: RichTextDocument;
  messages: RichTextMessages;
  contentLocale: string;
  direction?: RichTextDirection;
  spellcheck?: boolean;
  disabled?: boolean;
  className?: string;
  onChange(document: RichTextDocument): void;
  onError?(code: string, message: string): void;
}

export const RichTextEditor = forwardRef<
  RichTextEditorHandle,
  RichTextEditorProps
>(function RichTextEditor(props, ref) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const controllerRef = useRef<RichTextFrameController>(null);
  const onChangeRef = useRef(props.onChange);
  const onErrorRef = useRef(props.onError);
  onChangeRef.current = props.onChange;
  onErrorRef.current = props.onError;

  useImperativeHandle(ref, () => ({
    focus: () => controllerRef.current?.focus(),
    requestDocument: () => controllerRef.current?.requestDocument()
  }));

  useEffect(() => {
    const iframe = iframeRef.current;
    if (!iframe) return;
    const controller = connectRichTextFrame({
      iframe,
      frameUrl: props.frameUrl,
      profile: props.profile,
      document: props.value,
      messages: props.messages,
      contentLocale: props.contentLocale,
      direction: props.direction,
      spellcheck: props.spellcheck,
      editable: !props.disabled,
      onDocumentChange: (document) => onChangeRef.current(document),
      onError: (code, message) => onErrorRef.current?.(code, message)
    });
    controllerRef.current = controller;
    controller.ready.catch((error: unknown) => {
      onErrorRef.current?.(
        'frame_unavailable',
        error instanceof Error ? error.message : 'The editor frame failed to load.'
      );
    });
    return () => {
      controller.destroy();
      controllerRef.current = null;
    };
  }, [props.frameUrl, props.messages, props.profile]);

  useEffect(() => {
    controllerRef.current?.setDocument(props.value);
  }, [props.value]);

  useEffect(() => {
    controllerRef.current?.setEditable(!props.disabled);
  }, [props.disabled]);

  useEffect(() => {
    controllerRef.current?.setAuthoringContext({
      contentLocale: props.contentLocale,
      direction: props.direction,
      spellcheck: props.spellcheck
    });
  }, [props.contentLocale, props.direction, props.spellcheck]);

  return (
    <iframe
      ref={iframeRef}
      title={props.label}
      aria-disabled={props.disabled}
      sandbox='allow-scripts'
      className={props.className}
      referrerPolicy='no-referrer'
    />
  );
});
