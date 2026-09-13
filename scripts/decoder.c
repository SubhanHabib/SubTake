/* Shared FFmpeg decoder ABI. No window-system or capture APIs belong here. */
#include <libavformat/avformat.h>
#include <libavcodec/avcodec.h>
#include <libavutil/display.h>
#include <libavutil/time.h>
#include <libswscale/swscale.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef struct {
    AVFormatContext *format; AVCodecContext *codec; AVFrame *frame, *decoded; AVPacket *packet;
    struct SwsContext *scale; int stream, width, height, rotation, eof, have_frame;
    double last_time; int64_t deadline; uint8_t *rotated;
} SubTakeDecoder;
static int interrupt_read(void *opaque){SubTakeDecoder*d=opaque;return av_gettime_relative()>d->deadline;}
static int fail(int code,char*error,size_t capacity){if(code>=0)code=AVERROR_UNKNOWN;av_strerror(code,error,capacity);return -1;}
void subtake_decoder_close(SubTakeDecoder*d){if(!d)return;sws_freeContext(d->scale);av_free(d->rotated);av_frame_free(&d->frame);av_frame_free(&d->decoded);av_packet_free(&d->packet);avcodec_free_context(&d->codec);avformat_close_input(&d->format);free(d);}
SubTakeDecoder* subtake_decoder_open(const char*path,int width,int height,char*error,size_t capacity){
    if(width<=0||height<=0||width>8192||height>8192){fail(AVERROR(EINVAL),error,capacity);return NULL;}
    SubTakeDecoder*d=calloc(1,sizeof(*d));if(!d){fail(AVERROR(ENOMEM),error,capacity);return NULL;}
    d->width=width;d->height=height;d->last_time=-1;d->deadline=av_gettime_relative()+30000000;
    d->format=avformat_alloc_context();if(!d->format)goto allocation_failure;
    d->format->interrupt_callback=(AVIOInterruptCB){interrupt_read,d};
    int result=avformat_open_input(&d->format,path,NULL,NULL);if(result<0)goto failure;
    result=avformat_find_stream_info(d->format,NULL);if(result<0)goto failure;
    const AVCodec*codec=NULL;result=av_find_best_stream(d->format,AVMEDIA_TYPE_VIDEO,-1,-1,&codec,0);if(result<0)goto failure;d->stream=result;
    AVStream*stream=d->format->streams[d->stream];
    d->codec=avcodec_alloc_context3(codec);d->frame=av_frame_alloc();d->decoded=av_frame_alloc();d->packet=av_packet_alloc();if(!d->codec||!d->frame||!d->decoded||!d->packet)goto allocation_failure;
    result=avcodec_parameters_to_context(d->codec,stream->codecpar);if(result<0)goto failure;
    d->codec->thread_count=2;
    result=avcodec_open2(d->codec,codec,NULL);if(result<0)goto failure;
    const AVPacketSideData*side=av_packet_side_data_get(stream->codecpar->coded_side_data,stream->codecpar->nb_coded_side_data,AV_PKT_DATA_DISPLAYMATRIX);
    if(side && side->size>=9*sizeof(int32_t)){
        double angle=-av_display_rotation_get((const int32_t*)side->data);
        if(isfinite(angle))d->rotation=((int)lround(angle/90.)%4+4)%4;
    }
    if(d->rotation){d->rotated=av_malloc((size_t)width*height*4);if(!d->rotated)goto allocation_failure;}
    return d;
allocation_failure: result=AVERROR(ENOMEM);
failure:fail(result,error,capacity);subtake_decoder_close(d);return NULL;
}
static int next_frame(SubTakeDecoder*d){
    for(;;){int result=avcodec_receive_frame(d->codec,d->decoded);if(result>=0){av_frame_unref(d->frame);av_frame_move_ref(d->frame,d->decoded);return 0;}if(result!=AVERROR(EAGAIN))return result;
        if(d->eof)return AVERROR_EOF;
        do{result=av_read_frame(d->format,d->packet);if(result<0){if(result!=AVERROR_EOF)return result;d->eof=1;avcodec_send_packet(d->codec,NULL);break;}
            if(d->packet->stream_index==d->stream){result=avcodec_send_packet(d->codec,d->packet);av_packet_unref(d->packet);if(result<0)return result;break;}
            av_packet_unref(d->packet);
        }while(1);
    }
}
int subtake_decoder_frame(SubTakeDecoder*d,double seconds,uint8_t*rgba,size_t length,char*error,size_t capacity){
    if(!d || !isfinite(seconds) || seconds<0 || length<(size_t)d->width*d->height*4)return fail(AVERROR(EINVAL),error,capacity);
    AVStream*stream=d->format->streams[d->stream];double base=stream->start_time==AV_NOPTS_VALUE?0:stream->start_time*av_q2d(stream->time_base);
    double fps=av_q2d(av_guess_frame_rate(d->format,stream,NULL));if(!(fps>0 && isfinite(fps)))fps=30;
    d->deadline=av_gettime_relative()+30000000;
    if(!d->have_frame || seconds<d->last_time-0.5/fps || seconds-d->last_time>0.3){
        long double raw=((long double)seconds+base)/av_q2d(stream->time_base);
        if(!isfinite(raw)||raw>=9223372036854775808.0L||raw<=-9223372036854775808.0L)return fail(AVERROR(EINVAL),error,capacity);
        int64_t timestamp=(int64_t)llroundl(raw);
        int result=av_seek_frame(d->format,d->stream,timestamp,AVSEEK_FLAG_BACKWARD);if(result<0)return fail(result,error,capacity);
        avcodec_flush_buffers(d->codec);d->eof=0;d->have_frame=0;
    }
    while(!d->have_frame || d->last_time+0.5/fps<seconds){
        int result=next_frame(d);if(result<0){if(result==AVERROR_EOF && d->have_frame)break;return fail(result,error,capacity);}
        int64_t pts=d->frame->best_effort_timestamp;
        d->last_time=pts==AV_NOPTS_VALUE?(d->last_time<0?seconds:d->last_time+1./fps):pts*av_q2d(stream->time_base)-base;
        d->have_frame=1;
    }
    int width=(d->rotation%2)?d->height:d->width;int height=(d->rotation%2)?d->width:d->height;
    d->scale=sws_getCachedContext(d->scale,d->frame->width,d->frame->height,(enum AVPixelFormat)d->frame->format,width,height,AV_PIX_FMT_RGBA,SWS_BILINEAR,NULL,NULL,NULL);
    if(!d->scale)return fail(AVERROR(ENOMEM),error,capacity);
    int space=SWS_CS_DEFAULT;if(d->frame->colorspace==AVCOL_SPC_BT709)space=SWS_CS_ITU709;else if(d->frame->colorspace==AVCOL_SPC_BT2020_NCL)space=SWS_CS_BT2020;
    sws_setColorspaceDetails(d->scale,sws_getCoefficients(space),d->frame->color_range==AVCOL_RANGE_JPEG,sws_getCoefficients(space),1,0,1<<16,1<<16);
    uint8_t*output[4]={d->rotation?d->rotated:rgba,NULL,NULL,NULL};int stride[4]={width*4,0,0,0};
    int result=sws_scale(d->scale,(const uint8_t*const*)d->frame->data,d->frame->linesize,0,d->frame->height,output,stride);if(result<0)return fail(result,error,capacity);
    if(d->rotation){for(int y=0;y<d->height;y++)for(int x=0;x<d->width;x++){
        int sx=x,sy=y;
        if(d->rotation==1){sx=y;sy=height-1-x;}else if(d->rotation==2){sx=width-1-x;sy=height-1-y;}else if(d->rotation==3){sx=width-1-y;sy=x;}
        memcpy(rgba+((size_t)y*d->width+x)*4,d->rotated+((size_t)sy*width+sx)*4,4);
    }}
    return 0;
}
