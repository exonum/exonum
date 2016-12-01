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
            $.fancybox({
                href:this.hash,
                wrapCSS:'firas',
                closeSpeed:150,
                helpers: {
                    overlay : {
                        closeClick : false,  // if true, fancyBox will be closed when user clicks on the overlay
                        speedOut   : 200,   // duration of fadeOut animation
                        showEarly  : true,  // indicates if should be opened immediately or wait until the content is ready
                        css        : {'background':'rgba(255,255,255,0.5)'},    // custom CSS properties
                        locked     : true   // if true, the content will be locked into overlay
                    },
                    title : {
                        type : 'float' // 'float', 'inside', 'outside' or 'over'
                    }
                },
                tpl:{
                    wrap     : '<div class="fancybox-wrap " tabIndex="-1"><div class="fancybox-skin container"><div class="fancybox-outer"><div class="fancybox-inner"></div></div></div></div>',
                    closeBtn : '<a href="javascript:;" class="active_bg_close close_btn"><i class="pi pixicon-cross2"></i></a>',
                }
            });
            return false;
        });
        // $("[class*='_open']").click(function() {
        //     alert(this.hash);
        // //$.fancybox("#hidden_pix_1");
        // });

        // $('.pix_popup').popup({
        //   color: '#2dc0e8',
        //   opacity: 0.5,
        //   transition: '0.3s',
        //   blur:false,
        //   pagecontainer:'#page',
        //   scrolllock: true
        // });


        //======================================================================================================
        //      Go To Top
        //======================================================================================================
        $('#gototop').click(function(e){
            jQuery('html, body').animate({scrollTop:0}, 750, 'linear');
            e.preventDefault();
            return false;
        });
        //======================================================================================================
    

        $( "form, fieldset" ).live( "submit", function( event ) {
            event.preventDefault();
            var values = {};
            var temp_str = "";
            var theform = this;
            var proceed = true;
            $("input, textarea, select").css('border-color',''); 
            $.each($(theform).serializeArray(), function(i, field) {
                values[field.name] = field.value;
                temp_str += field.name + ": " + field.value + "\n";
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
            //alert(temp_str);
            if(proceed) 
            {
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
                        $.fancybox("#hidden_pix_6");
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