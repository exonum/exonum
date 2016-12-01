// jQuery Initialization
jQuery(document).ready(function($){
"use strict"; 


        //======================================================================================================
        //      Fancy Box
        //======================================================================================================
        if ($('.lightbox, .button-fullsize, .fullsize').length > 0) {
            $('.lightbox, .button-fullsize, .fullsize').fancybox({
                padding    : 0,
                margin    : 0,
                maxHeight  : '90%',
                maxWidth   : '90%',
                loop       : true,
                fitToView  : false,
                mouseWheel : false,
                autoSize   : false,
                closeClick : false,
                overlay    : { showEarly  : true },
                helpers    : { media : {} }
            });
        }
        //======================================================================================================

       



        // ----------------- EASING ANCHORS ------------------ //

        $('a[href*=#href]').live("click", function(){ 
         if (location.pathname.replace(/^\//,'') === this.pathname.replace(/^\//,'') && location.hostname === this.hostname) {
                 var $target = $(this.hash);
                 $target = $target.length && $target || $('[name=' + this.hash.slice(1) +']');
                 if ($target.length) {
                     var targetOffset = $target.offset().top;
                     $('html,body').animate({scrollTop: targetOffset-100}, 1000);
                     return false;
                }
           }
       });

        $('a[href*=#popup_]').live("click", function(){ 
            
            // alert(this.hash);
            // alert(this.name);
            var pix_over = 'rgba(0,0,0,0.5)';
            if($(this.hash).attr('pix-overlay')){
                pix_over = $(this.hash).attr('pix-overlay');
                //alert($(this.hash).attr('pix-pix-overlay'));
            }
            var pix_class = '';
            if($(this.hash).attr('pix-class')){
                pix_class = $(this.hash).attr('pix-class');
            }
            this.overlay = pix_over;
            $.fancybox({
                href:this.hash,
                wrapCSS:'firas',
                closeSpeed:150,
                helpers: {
                    overlay : {
                        closeClick : false,  // if true, fancyBox will be closed when user clicks on the overlay
                        speedOut   : 200,   // duration of fadeOut animation
                        showEarly  : true,  // indicates if should be opened immediately or wait until the content is ready
                        css        : {'background':pix_over},    // custom CSS properties
                        locked     : true   // if true, the content will be locked into overlay
                    },
                    title : {
                        type : 'float' // 'float', 'inside', 'outside' or 'over'
                    }
                },
                tpl:{
                    wrap     : '<div class="fancybox-wrap " tabIndex="-1"><div class="fancybox-skin container  '+ pix_class +'"><div class="fancybox-outer"><div class="fancybox-inner"></div></div></div></div>',
                    closeBtn : '<a href="javascript:;" class="active_bg_close close_btn"><i class="pi pixicon-cross2"></i></a>',
                }
            });
            return false;
        });


        //======================================================================================================
        //      Go To Top
        //======================================================================================================
        $('#gototop').click(function(e){
            jQuery('html, body').animate({scrollTop:0}, 750, 'linear');
            e.preventDefault();
            return false;
        });
        //======================================================================================================
    

        $("form").live( "submit", function( event ) {
            event.preventDefault();
            var values = {};
            var temp_str = "";
            var theform = this;
            var proceed = true;
            var is_confirm = false;
            var confirm_pop = "";
            var is_redirect = false;
            var redirect_link = "";
            var have_type = false;
            var the_type = "";
            if($(theform).attr('pix-confirm')){
                confirm_pop = $(theform).attr('pix-confirm');
                is_confirm = true;
            }
            if($(theform).attr('pix-redirect')){
                redirect_link = $(theform).attr('pix-redirect');
                is_redirect = true;
            }
            if($(theform).attr('pix-form-type')){
                if(($(theform).attr('pix-form-type')!='') && ($(theform).attr('pix-form-type')!='#' )){
                    the_type = $(theform).attr('pix-form-type');
                    have_type = true;    
                }
            }
            
            $("input, textarea, select").css('border-color',''); 
            $.each($(theform).serializeArray(), function(i, field) {
                values[field.name] = field.value;
                //temp_str += field.name + ": " + field.value + "\n";
                var is_required =$(theform).find('[name='+field.name+']').attr('required');
                //alert("THE FORM IS: "+sada);
                if(field.value=="" && is_required){
                    $(theform).find('input[name='+field.name+']').css('border-color','red');     
                    $(theform).find('textarea[name='+field.name+']').css('border-color','red');     
                    $(theform).find('select[name='+field.name+']').css('border-color','red'); 
                    proceed = false;
                }
                 //alert(this.name);
            });
            // if(is_confirm){
            //     $.fancybox($("#" + confirm_pop));
            // }
            //alert(temp_str);
            if(proceed) 
            {   
                if(have_type){
                    values['pixfort_form_type'] = the_type;
                    //alert(the_type);
                }
                //data to be sent to server
                var post_data;
                var output;
                //Ajax post data to server
                $.post('pix_mail/new_contact.php', values, function(response){  
                    //load json data from server and output message     
                    if(response.type == 'error')
                    {
                        output = '<div class="error">'+response.text+'</div>';
                    }else{
                        if(is_confirm){
                            $.fancybox($("#" + confirm_pop));
                        }
                        if(is_redirect){
                            window.location.href = redirect_link;
                        }
                        output = '<div class="success">'+response.text+'</div>';
                        //reset values in all input fields
                        $(theform).find('input').val(''); 
                        $(theform).find('textarea').val(''); 
                    }
                    $(theform).find('#result').hide().html(output).slideDown();
                }, 'json');
                
            }
            //alert( $( this ).serialize() );
        });
        $("input, textarea,  select").keyup(function() { 
            $(this).css('border-color',''); 
            $('#result').slideUp();
        });



        //======================================================================================================
        //  END OF DOCUMENT
        //=================
});